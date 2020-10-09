/*
   +----------------------------------------------------------------------+
   | HipHop for PHP                                                       |
   +----------------------------------------------------------------------+
   | Copyright (c) 2010-present Facebook, Inc. (http://www.facebook.com)  |
   +----------------------------------------------------------------------+
   | This source file is subject to version 3.01 of the PHP license,      |
   | that is bundled with this package in the file LICENSE, and is        |
   | available through the world-wide-web at the following url:           |
   | http://www.php.net/license/3_01.txt                                  |
   | If you did not receive a copy of the PHP license and are unable to   |
   | obtain it through the world-wide-web, please send a note to          |
   | license@php.net so we can mail you a copy immediately.               |
   +----------------------------------------------------------------------+
*/

#include "hphp/runtime/vm/jit/check.h"

#include "hphp/runtime/vm/jit/analysis.h"
#include "hphp/runtime/vm/jit/block.h"
#include "hphp/runtime/vm/jit/cfg.h"
#include "hphp/runtime/vm/jit/id-set.h"
#include "hphp/runtime/vm/jit/ir-instruction.h"
#include "hphp/runtime/vm/jit/ir-opcode.h"
#include "hphp/runtime/vm/jit/ir-unit.h"
#include "hphp/runtime/vm/jit/state-vector.h"
#include "hphp/runtime/vm/jit/phys-reg.h"
#include "hphp/runtime/vm/jit/reg-alloc.h"
#include "hphp/runtime/vm/jit/type.h"

#include "hphp/runtime/base/bespoke-array.h"
#include "hphp/runtime/base/perf-warning.h"
#include "hphp/runtime/ext/std/ext_std_closure.h"

#include <folly/Format.h>

#include <bitset>
#include <iostream>
#include <string>

#include <boost/dynamic_bitset.hpp>

namespace HPHP { namespace jit {

//////////////////////////////////////////////////////////////////////

namespace {

//////////////////////////////////////////////////////////////////////

TRACE_SET_MOD(hhir);

/*
 * Return the number of parameters required for this block.
 */
DEBUG_ONLY static int numBlockParams(Block* b) {
  return b->empty() || b->front().op() != DefLabel ? 0 :
         b->front().numDsts();
}

/*
 * Check one block for being well formed. Invariants verified:
 * 1. The block begins with an optional DefLabel, followed by an optional
 *    BeginCatch.
 * 2. DefLabel and BeginCatch may not appear anywhere in a block other than
 *    where specified in #1.
 * 3. If this block is a catch block, it must have at most one predecessor.
 * 4. The last instruction must be isBlockEnd() and the middle instructions
 *    must not be isBlockEnd().  Therefore, blocks cannot be empty.
 * 5. block->next() must be null iff the last instruction isTerminal().
 * 6. Every instruction must have a catch block attached to it if and only if it
 *    has the MayRaiseError flag.
 * 7. Any path from this block to a Block that expects values must be
 *    from a Jmp instruciton.
 * 8. Every instruction's BCMarker must point to a valid bytecode instruction.
 */
bool checkBlock(Block* b) {
  SCOPE_ASSERT_DETAIL("checkBlock") { return folly::sformat("B{}", b->id()); };
  auto it = b->begin();
  auto end = b->end();
  always_assert(!b->empty());

  // Invariant #1
  if (it->op() == DefLabel) {
    ++it;
  }

  // Invariant #1
  if (it != end && it->op() == BeginCatch) {
    ++it;
  }

  // Invariants #2, #4
  always_assert(it != end && b->back().isBlockEnd());
  --end;
  for (IRInstruction& inst : folly::range(it, end)) {
    always_assert(inst.op() != DefLabel);
    always_assert(inst.op() != BeginCatch);
    always_assert(!inst.isBlockEnd());
  }
  for (IRInstruction& inst : *b) {
    // Invariant #8
    always_assert(inst.marker().valid());
    always_assert(inst.block() == b);
    // Invariant #6
    always_assert_flog(
      inst.mayRaiseError() == (inst.taken() && inst.taken()->isCatch()),
      "{}", inst
    );
  }

  // Invariant #5
  always_assert(b->back().isTerminal() == (b->next() == nullptr));

  // Invariant #7
  if (b->taken()) {
    // only Jmp can branch to a join block expecting values.
    IRInstruction* branch = &b->back();
    auto numArgs = branch->op() == Jmp ? branch->numSrcs() : 0;
    always_assert(numBlockParams(b->taken()) == numArgs);
  }

  // Invariant #3
  if (b->isCatch()) {
    // keyed off a tca, so there needs to be exactly one
    always_assert(b->preds().size() <= 1);
  }

  return true;
}

///////////////////////////////////////////////////////////////////////////////

}

///////////////////////////////////////////////////////////////////////////////

/*
 * Build the CFG, then the dominator tree, then use it to validate SSA.
 * 1. Each src must be defined by some other instruction, and each dst must
 *    be defined by the current instruction.
 * 2. Each src must be defined earlier in the same block or in a dominator.
 * 3. Each dst must not be previously defined.
 * 4. Treat tmps defined by DefConst as always defined.
 * 5. Each predecessor of a reachable block must be reachable (deleted
 *    blocks must not have out-edges to reachable blocks).
 * 6. The entry block must not have any predecessors.
 * 7. The entry block starts with a DefFP instruction.
 */
bool checkCfg(const IRUnit& unit) {
  auto const blocks = rpoSortCfg(unit);
  auto const rpoIDs = numberBlocks(unit, blocks);
  auto reachable    = boost::dynamic_bitset<>(unit.numBlocks());

  // Entry block can't have predecessors.
  always_assert(unit.entry()->numPreds() == 0);

  // Entry block starts with DefFP.
  always_assert(!unit.entry()->empty() &&
                unit.entry()->begin()->op() == DefFP);

  // Check valid successor/predecessor edges, and identify reachable blocks.
  for (Block* b : blocks) {
    reachable.set(b->id());
    auto checkEdge = [&] (const Edge* e) {
      always_assert(e->from() == b);
      for (auto& p : e->to()->preds()) if (&p == e) return;
      always_assert(false); // did not find edge.
    };
    checkBlock(b);
    if (auto e = b->nextEdge())  checkEdge(e);
    if (auto e = b->takenEdge()) checkEdge(e);
  }
  for (Block* b : blocks) {
    for (auto const& e : b->preds()) {
      always_assert(&e == e.inst()->takenEdge() || &e == e.inst()->nextEdge());
      always_assert(e.to() == b);

      // Invariant #5
      always_assert_flog(reachable.test(e.from()->id()),
        "unreachable: B{}\n", e.from()->id());
    }
  }

  auto defined_set = jit::sparse_idptr_set<SSATmp>{unit.numTmps()};

  /*
   * Visit every instruction and make sure their sources are either defined in
   * a block that strictly dominates the block containing the instruction, or
   * defined earlier in the same block as the instruction.
   */
  auto const idoms = findDominators(unit, blocks, rpoIDs);
  for (auto& blk : blocks) {
    for (auto& inst : blk->instrs()) {
      for (auto src : inst.srcs()) {
        if (src->inst()->is(DefConst)) continue;
        if (src->type() <= TBottom) continue;

        always_assert_flog(
          src->inst()->dsts().contains(src),
          "src '{}' has '{}' as its instruction, "
          "but the instruction does not have '{}' as a dst",
          src->toString(),
          src->inst()->toString(),
          src->toString()
        );

        auto const dom = findDefiningBlock(src, idoms);
        auto const locally_defined =
          src->inst()->block() == inst.block() && defined_set.contains(src);
        auto const strictly_dominates =
          src->inst()->block() != inst.block() &&
          dom && dominates(dom, inst.block(), idoms);
        always_assert_flog(
          locally_defined || strictly_dominates,
          "src '{}' in '{}' came from '{}', which is not a "
          "DefConst and is not defined at this use site",
          src->toString(), inst.toString(),
          src->inst()->toString()
        );
      }
      for (auto dst : inst.dsts()) defined_set.insert(dst);
    }
    defined_set.clear();
  }

  /*
   * Check that each dst is defined only once.
   */
  defined_set.clear();
  for (auto& blk : blocks) {
    for (auto& inst : blk->instrs()) {
      for (auto dst : inst.dsts()) {
        always_assert_flog(
          !defined_set.contains(dst),
          "SSATmp ({}) was defined multiple times",
          dst->toString()
        );
        defined_set.insert(dst);
      }
    }
  }

  return true;
}

bool checkTmpsSpanningCalls(const IRUnit& unit) {
  auto ignoreSrc = [&](IRInstruction& /*inst*/, SSATmp* src) {
    /*
     * FramePtr/StkPtr-typed tmps may live across calls.
     *
     * Tmps defined by DefConst are always available and may be assigned to
     * registers if needed by the instructions using the const.
     */
    return src->isA(TStkPtr) ||
           src->isA(TFramePtr) ||
           src->inst()->is(DefConst);
  };

  StateVector<Block,IdSet<SSATmp>> livein(unit, IdSet<SSATmp>());
  bool isValid = true;
  std::string failures;
  postorderWalk(unit, [&](Block* block) {
    auto& live = livein[block];
    if (auto taken = block->taken()) live = livein[taken];
    if (auto next  = block->next()) live |= livein[next];
    for (auto it = block->end(); it != block->begin();) {
      auto& inst = *--it;
      for (auto dst : inst.dsts()) {
        live.erase(dst);
      }
      if (isCallOp(inst.op())) {
        live.forEach([&](uint32_t tmp) {
          folly::format(&failures, "t{} is live across `{}`\n", tmp, inst);
          isValid = false;
        });
      }
      for (auto* src : inst.srcs()) {
        if (!ignoreSrc(inst, src)) live.add(src);
      }
    }
  });

  if (!isValid) {
    logLowPriPerfWarning(
      "checkTmpsSpanningCalls",
      100 * kDefaultPerfWarningRate,
      [&](StructuredLogEntry& cols) {
        cols.setStr("live_tmps", failures);
        cols.setStr("hhir_unit", show(unit));
      }
    );
  }
  return isValid;
}

///////////////////////////////////////////////////////////////////////////////
// checkOperandTypes().

namespace {

/*
 * Return a union type containing all the types in the argument list.
 */
Type buildUnion() {
  return TBottom;
}

template<class... Args>
Type buildUnion(Type t, Args... ts) {
  return t | buildUnion(ts...);
}

template <uint32_t...> struct IdxSeq {};

template <typename F>
inline void forEachSrcIdx(F /*f*/, IdxSeq<>) {}

template <typename F, uint32_t Idx, uint32_t... Rest>
inline void forEachSrcIdx(F f, IdxSeq<Idx, Rest...>) {
  f(Idx); forEachSrcIdx(f, IdxSeq<Rest...>{});
}

}

/*
 * Runtime typechecking for IRInstruction operands.
 *
 * This is generated using the table in ir-opcode.h.  We instantiate
 * IR_OPCODES after defining all the various source forms to do type
 * assertions according to their form (see ir-opcode.h for documentation on
 * the notation).  The checkers appear in argument order, so each one
 * increments curSrc, and at the end we can check that the argument
 * count was also correct.
 */
bool checkOperandTypes(const IRInstruction* inst, const IRUnit* /*unit*/) {
  int curSrc = 0;

  auto bail = [&] (const std::string& msg) {
    FTRACE(1, "{}", msg);
    fprintf(stderr, "%s\n", msg.c_str());

    always_assert_log(false, [&] { return msg; });
  };

  if (opHasExtraData(inst->op()) != (bool)inst->rawExtra()) {
    bail(folly::format("opcode {} should{} have an ExtraData struct "
                       "but instruction {} does{}",
                       inst->op(),
                       opHasExtraData(inst->op()) ? "" : "n't",
                       *inst,
                       inst->rawExtra() ? "" : "n't").str());
  }

  auto src = [&]() -> SSATmp* {
    if (curSrc < inst->numSrcs()) {
      return inst->src(curSrc);
    }

    bail(folly::format(
      "Error: instruction had too few operands\n"
      "   instruction: {}\n",
        inst->toString()
      ).str()
    );
    not_reached();
  };

  // If expected is not nullptr, it will be used. Otherwise, t.toString() will
  // be used as the expected string.
  auto check = [&] (bool cond, const Type t, const char* expected) {
    if (cond) return true;

    std::string expectStr = expected ? expected : t.toString();

    bail(folly::format(
      "Error: failed type check on operand {}\n"
      "   instruction: {}\n"
      "   was expecting: {}\n"
      "   received: {}\n"
      "   from: {}\n",
        curSrc,
        *inst,
        expectStr,
        inst->src(curSrc)->type(),
        *inst->src(curSrc)->inst()
      ).str()
    );
    return true;
  };

  auto checkNoArgs = [&]{
    if (inst->numSrcs() == 0) return true;
    bail(folly::format(
      "Error: instruction expected no operands\n"
      "   instruction: {}\n",
        inst->toString()
      ).str()
    );
    return true;
  };

  auto countCheck = [&]{
    if (inst->numSrcs() == curSrc) return true;
    bail(folly::format(
      "Error: instruction had too many operands\n"
      "   instruction: {}\n"
      "   expected {} arguments\n",
        inst->toString(),
        curSrc
      ).str()
    );
    return true;
  };

  auto checkArr = [&] (bool is_kv, bool is_const) {
    auto const t = src()->type();
    auto const cond_type = RuntimeOption::EvalHackArrDVArrs
      ? (is_kv ? TDict : TVec)
      : (is_kv ? TDArr : TVArr);
    if (is_const) {
      auto expected = folly::sformat("constant {}", t.toString());
      check(src()->hasConstVal(cond_type), t, expected.c_str());
    } else {
      check(src()->isA(cond_type), t, nullptr);
    }
    ++curSrc;
  };

  auto checkDst = [&] (bool cond, const std::string& errorMessage) {
    if (cond) return true;

    bail(folly::format("Error: failed type check on dest operand\n"
                       "   instruction: {}\n"
                       "   message: {}\n",
                       inst->toString(),
                       errorMessage).str());
    return true;
  };

  auto requireTypeParam = [&] (Type ty) {
    checkDst(inst->hasTypeParam() || inst->is(DefConst),
             "Missing paramType for DParam instruction");
    if (inst->hasTypeParam()) {
      checkDst(inst->typeParam() <= ty,
               "Invalid paramType for DParam instruction");
    }
  };

  auto checkConstant = [&] (SSATmp* src, Type type, const char* expected) {
    // We can't check src->hasConstVal(type) because of TNullptr.
    auto const match = src->isA(type) && src->type().admitsSingleVal();
    check(match || src->isA(TBottom), type, expected);
  };

  // If the bespoke runtime check flag is off, leave the IR types unchanged.
  // Otherwise, assume that non-layout-agnostic ops taking an S(Arr) actually
  // take an S(VanillaArr), and likewise for other array-likes.
  auto const checkLayoutFlags = [&] (std::vector<Type> types) {
    if (allowBespokeArrayLikes() &&
        !inst->isLayoutAgnostic() &&
        LIKELY(!RO::EvalAllowBespokesInLiveTypes)) {
      for (auto& type : types) type = type.narrowToVanilla();
    }
    return types;
  };
  auto const getTypeNames = [&] (const std::vector<Type>& types) {
    auto parts = std::vector<std::string>{};
    for (auto const& type : types) parts.push_back(type.toString());
    return folly::join(" or ", parts);
  };
  auto const checkMultiple = [&] (SSATmp* src, const std::vector<Type>& types,
                                  const std::string& message) {
    auto okay = false;
    for (auto const& type : types) if ((okay = src->isA(type))) break;
    check(okay, Type(), message.c_str());
  };

using namespace TypeNames;
using TypeNames::TCA;

#define NA            return checkNoArgs();
#define S(T...)       {                                                     \
                        static auto const types = checkLayoutFlags({T});    \
                        static auto const names = getTypeNames(types);      \
                        checkMultiple(src(), types, names);                 \
                        ++curSrc;                                           \
                      }
#define AK(kind)      Type::Array(ArrayData::k##kind##Kind)
#define C(T)          checkConstant(src(), T, "constant " #T); ++curSrc;
#define CStr          C(StaticStr)
#define SVar(T...)    {                                                     \
                        static auto const types = checkLayoutFlags({T});    \
                        static auto const names = getTypeNames(types);      \
                        for (; curSrc < inst->numSrcs(); ++curSrc) {        \
                          checkMultiple(src(), types, names);               \
                        }                                                   \
                      }
#define SVArr         checkArr(false /* is_kv */, false /* is_const */);
#define SDArr         checkArr(true  /* is_kv */, false /* is_const */);
#define CDArr         checkArr(true  /* is_kv */, true  /* is_const */);
#define ND
#define DMulti
#define DSetElem
#define D(...)
#define DBuiltin
#define DCall
#define DGenIter
#define DSubtract(src, t)checkDst(src < inst->numSrcs(),  \
                             "invalid src num");
#define DofS(src)   checkDst(src < inst->numSrcs(),  \
                             "invalid src num");
#define DRefineS(src) checkDst(src < inst->numSrcs(),  \
                               "invalid src num");     \
                      requireTypeParam(Top);
#define DParam(t)      requireTypeParam(t);
#define DUnion(...)    forEachSrcIdx(                                          \
                         [&](uint32_t idx) {                                   \
                           checkDst(idx < inst->numSrcs(), "invalid src num"); \
                         },                                                    \
                         IdxSeq<__VA_ARGS__>{}                                 \
                       );
#define DLdObjCls
#define DAllocObj
#define DVecElem
#define DDictElem
#define DDictSet
#define DVecSet
#define DKeysetElem
#define DVecFirstElem
#define DVecLastElem
#define DVecKey
#define DDictFirstElem
#define DDictLastElem
#define DDictFirstKey
#define DDictLastKey
#define DKeysetFirstElem
#define DKeysetLastElem
#define DLoggingArrLike
#define DVArr
#define DDArr
#define DStaticDArr
#define DCol
#define DMemoKey
#define DLvalOfPtr
#define DPtrIter
#define DPtrIterVal

#define O(opcode, dstinfo, srcinfo, flags) \
  case opcode: dstinfo srcinfo countCheck(); return true;

  switch (inst->op()) {
    IR_OPCODES
  default: always_assert(false);
  }

#undef O

#undef NA
#undef S
#undef AK
#undef C
#undef CStr
#undef SVar
#undef SVArr
#undef SVArrOrNull
#undef SDArr
#undef CDArr

#undef ND
#undef D
#undef DBuiltin
#undef DCall
#undef DGenIter
#undef DSubtract
#undef DMulti
#undef DSetElem
#undef DofS
#undef DRefineS
#undef DParam
#undef DLdObjCls
#undef DAllocObj
#undef DVecElem
#undef DDictElem
#undef DDictSet
#undef DVecSet
#undef DKeysetElem
#undef DVecFirstElem
#undef DVecLastElem
#undef DVecKey
#undef DDictFirstElem
#undef DDictLastElem
#undef DDictFirstKey
#undef DDictLastKey
#undef DKeysetFirstElem
#undef DKeysetLastElem
#undef DLoggingArrLike
#undef DVArr
#undef DDArr
#undef DStaticDArr
#undef DCol
#undef DUnion
#undef DMemoKey
#undef DLvalOfPtr
  return true;
}

bool checkEverything(const IRUnit& unit) {
  assertx(checkCfg(unit));
  if (debug) {
    checkTmpsSpanningCalls(unit);
    forEachInst(rpoSortCfg(unit), [&](IRInstruction* inst) {
      assertx(checkOperandTypes(inst, &unit));
    });
  }
  return true;
}

//////////////////////////////////////////////////////////////////////

}}
