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
#include "hphp/hhbbc/compression.h"

#include "hphp/hhbbc/bc.h"
#include "hphp/util/trace.h"

#ifdef __GNUG__
#include <cxxabi.h>
#endif // _GNUG_

namespace HPHP { namespace HHBBC { namespace compression {

//////////////////////////////////////////////////////////////////////

namespace {

//////////////////////////////////////////////////////////////////////

TRACE_SET_MOD(hhbbc_mem);

constexpr int32_t kNoSrcLoc = -1;

constexpr uint8_t k16BitCode = 0xfe;
constexpr uint8_t k32BitCode = 0xff;

// HHBC uses "9-bit" opcodes...that is, we have more than 256 valid
// bytecode ops, but less than 512. How convenient!
constexpr uint8_t k9BitOpShift = 0xff;

template <typename>
struct is_compact_vector : std::false_type {};

template <typename T, typename A>
struct is_compact_vector<CompactVector<T, A>> : std::true_type {};

template <typename T> constexpr bool copy_as_bytes() {
  return std::is_trivially_copyable<T>::value ||
         std::is_same<T, LowStringPtr>::value ||
         std::is_same<T, SSwitchTabEnt>::value;
}

std::string name(const std::type_info& type) {
#ifdef __GNUG__
  auto length = size_t{0};
  auto status = int{0};
  std::unique_ptr<char, decltype(&std::free)> result(
    __cxxabiv1::__cxa_demangle(type.name(), nullptr, &length, &status),
    &std::free);
  return result.get();
#else
  return type.name();
#endif // _GNUG_
}

//////////////////////////////////////////////////////////////////////

template <typename T>
T decode_as_bytes(const Buffer& buffer, size_t& pos) {
  static_assert(copy_as_bytes<T>(), "");
  alignas(alignof(T)) char data[sizeof(T)];
  memmove(&data[0], &buffer[pos], sizeof(T));
  pos += sizeof(T);
  return *reinterpret_cast<const T*>(&data[0]);
}

#define DECODE_MEMBER(x) decode<decltype(std::declval<T>().x)>(buffer, pos)

template <typename T>
T decode(const Buffer& buffer, size_t& pos) {
  assertx(pos < buffer.size());
  ITRACE(4, "at {}: {}\n", pos, name(typeid(T)));
  Trace::Indent _;

  if constexpr (std::is_same<T, FCallArgs>::value) {
    using FCA = FCallArgsBase;
    auto const base     = decode<FCA>(buffer, pos);
    auto const context  = decode<LSString>(buffer, pos);
    auto const aeTarget = decode<BlockId>(buffer, pos) + NoBlockId;
    auto inout = std::unique_ptr<uint8_t[]>();
    if (base.flags & FCallArgsBase::EnforceInOut) {
      auto const bytes = (base.numArgs + 7) / 8;
      inout = std::make_unique<uint8_t[]>(bytes);
      memmove(inout.get(), &buffer[pos], bytes);
      pos += bytes;
    }
    return FCallArgs(static_cast<FCA::Flags>(base.flags & FCA::kInternalFlags),
                     base.numArgs, base.numRets, std::move(inout), aeTarget,
                     base.lockWhileUnwinding, base.skipNumArgsCheck, context);
  }

  if constexpr (std::is_same<T, IterArgs>::value) {
    auto const flags  = DECODE_MEMBER(flags);
    auto const iterId = DECODE_MEMBER(iterId);
    auto const keyId  = DECODE_MEMBER(keyId) + IterArgs::kNoKey;
    auto const valId  = DECODE_MEMBER(valId);
    return T(flags, iterId, keyId, valId);
  }

  if constexpr (std::is_same<T, LocalRange>::value) {
    auto const first = DECODE_MEMBER(first);
    auto const count = DECODE_MEMBER(count);
    return T{first, count};
  }

  if constexpr (std::is_same<T, MKey>::value) {
    auto const mcode = DECODE_MEMBER(mcode);
    switch (mcode) {
      case MET: case MPT: case MQT: return T(mcode, DECODE_MEMBER(litstr));
      case MEI: case MEC: case MPC: return T(mcode, DECODE_MEMBER(int64));
      case MEL: case MPL:           return T(mcode, DECODE_MEMBER(local));
      case MW:                      return T();
    }
  }

  if constexpr (std::is_same<T, NamedLocal>::value) {
    auto const base = safe_cast<int32_t>(decode<uint32_t>(buffer, pos));
    auto const name = base + kInvalidLocalName;
    auto const id   = DECODE_MEMBER(id) + NoLocalId;
    return T(name, id);
  }

  if constexpr (is_compact_vector<T>::value) {
    auto data = T(decode<uint32_t>(buffer, pos));
    for (auto& item : data) {
      using Item = typename std::remove_reference<decltype(item)>::type;
      item = decode<Item>(buffer, pos);
    }
    return data;
  }

  if constexpr (std::is_same<T, uint32_t>::value) {
    auto const byte = decode_as_bytes<uint8_t>(buffer, pos);
    return byte == k32BitCode ? decode_as_bytes<uint32_t>(buffer, pos) :
           byte == k16BitCode ? decode_as_bytes<uint16_t>(buffer, pos) : byte;
  }

  if constexpr (std::is_same<T, Op>::value) {
    static_assert(sizeof(Op) == sizeof(uint16_t), "");
    auto const byte = decode_as_bytes<uint8_t>(buffer, pos);
    if (byte < k9BitOpShift) return Op(byte);
    auto const next = decode_as_bytes<uint8_t>(buffer, pos);
    return Op(safe_cast<uint16_t>(next) + k9BitOpShift);
  }

  if constexpr (copy_as_bytes<T>()) {
    return decode_as_bytes<T>(buffer, pos);
  }
}

#undef DECODE_MEMBER

//////////////////////////////////////////////////////////////////////

template <typename T>
void encode_as_bytes(Buffer& buffer, const T& data) {
  static_assert(copy_as_bytes<T>(), "");
  auto const ptr = reinterpret_cast<const char*>(&data);
  buffer.insert(buffer.end(), ptr, ptr + sizeof(T));
}

template <typename T>
void encode(Buffer& buffer, const T& data) {
  ITRACE(4, "at {}: {}\n", buffer.size(), name(typeid(T)));
  Trace::Indent _;

  if constexpr (std::is_same<T, FCallArgs>::value) {
    auto base = data.base();
    if (data.enforceInOut()) {
      auto const flags = base.flags | FCallArgsBase::EnforceInOut;
      base.flags = static_cast<FCallArgsBase::Flags>(flags);
    }
    encode(buffer, base);
    encode(buffer, data.context());
    encode(buffer, data.asyncEagerTarget() - NoBlockId);
    if (data.enforceInOut()) {
      auto const start = buffer.size();
      auto const bytes = (data.numArgs() + 7) / 8;
      buffer.insert(buffer.end(), bytes, 0);
      for (auto i = 0; i < data.numArgs(); i++) {
        if (data.isInOut(i)) buffer[start + (i / 8)] |= (1 << (i % 8));
      }
    }

  } else if constexpr (std::is_same<T, IterArgs>::value) {
    encode(buffer, data.flags);
    encode(buffer, data.iterId);
    encode(buffer, data.keyId - IterArgs::kNoKey);
    encode(buffer, data.valId);

  } else if constexpr (std::is_same<T, LocalRange>::value) {
    encode(buffer, data.first);
    encode(buffer, data.count);

  } else if constexpr (std::is_same<T, MKey>::value) {
    encode(buffer, data.mcode);
    switch (data.mcode) {
      case MET: case MPT: case MQT: encode(buffer, data.litstr); break;
      case MEI: case MEC: case MPC: encode(buffer, data.int64);  break;
      case MEL: case MPL:           encode(buffer, data.local);  break;
      case MW:                                                   break;
    }

  } else if constexpr (std::is_same<T, NamedLocal>::value) {
    encode(buffer, safe_cast<uint32_t>(data.name - kInvalidLocalName));
    encode(buffer, data.id - NoLocalId);

  } else if constexpr (is_compact_vector<T>::value) {
    encode(buffer, safe_cast<uint32_t>(data.size()));
    for (auto const& item : data) encode(buffer, item);

  } else if constexpr (std::is_same<T, uint32_t>::value) {
    if (data < std::min(k16BitCode, k32BitCode)) {
      encode_as_bytes(buffer, safe_cast<uint8_t>(data));
    } else if (data <= std::numeric_limits<uint16_t>::max()) {
      encode_as_bytes(buffer, k16BitCode);
      encode_as_bytes(buffer, safe_cast<uint16_t>(data));
    } else {
      encode_as_bytes(buffer, k32BitCode);
      encode_as_bytes(buffer, data);
    }

  } else if constexpr (std::is_same<T, Op>::value) {
    static_assert(sizeof(Op) == sizeof(uint16_t), "");
    auto const raw = uint16_t(data);
    if (raw < k9BitOpShift) {
      encode_as_bytes(buffer, safe_cast<uint8_t>(raw));
    } else {
      encode_as_bytes(buffer, k9BitOpShift);
      encode_as_bytes(buffer, safe_cast<uint8_t>(raw - k9BitOpShift));
    }

  } else if constexpr (copy_as_bytes<T>()) {
    encode_as_bytes(buffer, data);
  }
}

//////////////////////////////////////////////////////////////////////

}

//////////////////////////////////////////////////////////////////////

#define IMM_NA
#define IMM_ONE(x)                IMM(x, 1)
#define IMM_TWO(x, y)             IMM_ONE(x) IMM(y, 2)
#define IMM_THREE(x, y, z)        IMM_TWO(x, y) IMM(z, 3)
#define IMM_FOUR(x, y, z, n)      IMM_THREE(x, y, z) IMM(n, 4)
#define IMM_FIVE(x, y, z, n, m)   IMM_FOUR(x, y, z, n) IMM(m, 5)
#define IMM_SIX(x, y, z, n, m, o) IMM_FIVE(x, y, z, n, m) IMM(o, 6)

void decodeBytecodeVec(const Buffer& buffer, BytecodeVec& bcs) {
  FTRACE(2, "\ndecodeBytecodeVec: {} bytes\n", buffer.size());
  Trace::Indent _;
  auto pos = size_t{0};

#define IMM(type, n) \
  decode<decltype(std::declval<T>().IMM_NAME_##type(n))>(buffer, pos),
#define O(op, imms, ...)           \
    auto const decode_##op = [&] { \
      using T = bc::op;            \
      return T { IMM_##imms };     \
    };
    OPCODES
#undef O
#undef IMM

  bcs.resize(decode<uint32_t>(buffer, pos));
  for (auto& inst : bcs) {
    inst.op = decode<Op>(buffer, pos);
    inst.srcLoc = safe_cast<int32_t>(decode<uint32_t>(buffer, pos)) + kNoSrcLoc;
    ITRACE(3, "at {}: {}:\n", pos, opcodeToName(inst.op));
    Trace::Indent _;
#define O(op, ...) \
  case Op::op: new (&inst.op) bc::op(decode_##op()); break;
    switch (inst.op) { OPCODES }
#undef O
  }
  assertx(pos == buffer.size());
}

void encodeBytecodeVec(Buffer& buffer, const BytecodeVec& bcs) {
  FTRACE(2, "\nencodeBytecodeVec: {} elements\n", bcs.size());
  Trace::Indent _;

#define IMM(type, n) encode(buffer, data.IMM_NAME_##type(n));
#define O(op, imms, ...)                               \
    auto const encode_##op = [&](const bc::op& data) { \
      IMM_##imms                                       \
    };
    OPCODES
#undef O
#undef IMM

  encode(buffer, safe_cast<uint32_t>(bcs.size()));
  for (auto const& inst : bcs) {
    encode(buffer, inst.op);
    encode(buffer, safe_cast<uint32_t>(inst.srcLoc - kNoSrcLoc));
    ITRACE(3, "at {}: {}\n", buffer.size(), opcodeToName(inst.op));
    Trace::Indent _;
#define O(op, ...) case Op::op: encode_##op(inst.op); break;
    switch (inst.op) { OPCODES }
#undef O
  }
}

#undef IMM_NA
#undef IMM_ONE
#undef IMM_TWO
#undef IMM_THREE
#undef IMM_FOUR
#undef IMM_FIVE
#undef IMM_SIX

//////////////////////////////////////////////////////////////////////

void testCompression(php::Program& program) {
  trace_time tracer("test compression");
  auto total_full_size = size_t{0};
  auto total_compressed_size = size_t{0};
  auto temp = BytecodeVec{};
  auto buffer = Buffer{};

  auto test_compression_function = [&](php::Func& func) {
    auto mf = php::MutFunc(&func);
    for (auto& block : mf.blocks_mut()) {
      auto const old_size = block->hhbcs.size() * sizeof(block->hhbcs[0]);
      compression::encodeBytecodeVec(buffer, block->hhbcs);
      auto result = block.mutate();
      std::swap(temp, result->hhbcs);
      result->hhbcs.clear();
      compression::decodeBytecodeVec(buffer, result->hhbcs);
      auto const new_size = buffer.size();

      SCOPE_ASSERT_DETAIL("test_compression") {
        auto const new_block = show(func, *result);
        std::swap(temp, result->hhbcs);
        auto const old_block = show(func, *result);
        return folly::format("Original:\n\n{}\nFinal:\n\n{}\n",
                             old_block, new_block).str();
      };
      always_assert(temp.size() == result->hhbcs.size());
      for (auto i = 0; i < temp.size(); i++) {
        SCOPE_ASSERT_DETAIL("test_compression_bytecode") {
          return folly::format("Original:\n{}\n\nFinal:\n{}",
                               show(func, temp[i]),
                               show(func, result->hhbcs[i])).str();
        };
        always_assert(temp[i] == result->hhbcs[i]);
      }
      total_full_size += old_size;
      total_compressed_size += new_size;
      buffer.clear();
    }
  };

  for (auto& unit : program.units) {
    for (auto& c : unit->classes) {
      for (auto& m : c->methods) {
        test_compression_function(*m);
      }
    }
    for (auto& f : unit->funcs) {
      test_compression_function(*f);
    }
  }

  TRACE(1, "Overall compression ratio: %.2f\n",
        1.0 * total_full_size / std::max(total_compressed_size, size_t{1}));
}

//////////////////////////////////////////////////////////////////////

}}}
