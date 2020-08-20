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

#include "hphp/runtime/base/bespoke/logging-array.h"

#include "hphp/runtime/base/array-data-defs.h"
#include "hphp/runtime/base/bespoke-array.h"
#include "hphp/runtime/base/memory-manager.h"
#include "hphp/runtime/base/memory-manager-defs.h"
#include "hphp/runtime/base/mixed-array-defs.h"
#include "hphp/runtime/base/runtime-option.h"
#include "hphp/runtime/vm/jit/mcgen-translate.h"
#include "hphp/runtime/server/memory-stats.h"
#include "hphp/runtime/vm/vm-regs.h"

#include <tbb/concurrent_hash_map.h>
#include <atomic>

namespace HPHP { namespace bespoke {

TRACE_SET_MOD(bespoke);

//////////////////////////////////////////////////////////////////////////////

struct LoggingProfile {
  explicit LoggingProfile(SrcKey key)
    : srckey(key)
    , sampleCount(0)
    , staticArray(nullptr) {};

  SrcKey srckey;
  std::atomic<uint64_t> sampleCount;
  LoggingArray* staticArray;
};

namespace {

constexpr size_t kSizeIndex = 1;
static_assert(kSizeIndex2Size[kSizeIndex] >= sizeof(LoggingArray),
              "kSizeIndex must be large enough to fit a LoggingArray");
static_assert(kSizeIndex == 0 ||
              kSizeIndex2Size[kSizeIndex - 1] < sizeof(LoggingArray),
              "kSizeIndex must be the smallest size for LoggingArray");

LoggingLayout* s_layout = new LoggingLayout();
std::atomic<bool> g_loggingEnabled;

using ProfileMap =
  tbb::concurrent_hash_map<SrcKey, LoggingProfile*, SrcKey::TbbHashCompare>;
ProfileMap s_profileMap;

// The bespoke kind for a vanilla kind. Assumes the kind supports bespokes.
HeaderKind getBespokeKind(ArrayData::ArrayKind kind) {
  switch (kind) {
    case ArrayData::kPackedKind: return HeaderKind::BespokeVArray;
    case ArrayData::kMixedKind:  return HeaderKind::BespokeDArray;
    case ArrayData::kVecKind:    return HeaderKind::BespokeVec;
    case ArrayData::kDictKind:   return HeaderKind::BespokeDict;
    case ArrayData::kKeysetKind: return HeaderKind::BespokeKeyset;

    case ArrayData::kBespokeVArrayKind:
    case ArrayData::kBespokeDArrayKind:
    case ArrayData::kBespokeVecKind:
    case ArrayData::kBespokeDictKind:
    case ArrayData::kBespokeKeysetKind:
    case ArrayData::kNumKinds:
      always_assert(false);
  }
  not_reached();
}

LoggingArray* makeWithProfile(ArrayData* ad, LoggingProfile* prof) {
  assertx(ad->isVanilla());
  assertx(ad->getPosition() == ad->iter_begin());

  auto lad = static_cast<LoggingArray*>(tl_heap->objMallocIndex(kSizeIndex));
  lad->initHeader_16(getBespokeKind(ad->kind()), OneReference,
                     ad->isLegacyArray() ? ArrayData::kLegacyArray : 0);
  lad->setLayout(s_layout);
  lad->wrapped = ad;
  lad->profile = prof;
  assertx(lad->checkInvariants());
  return lad;
}
}

void setLoggingEnabled(bool val) {
  g_loggingEnabled.store(val, std::memory_order_relaxed);
}

ArrayData* maybeEnableLogging(ArrayData* ad) {
  if (!g_loggingEnabled.load(std::memory_order_relaxed)) return ad;
  VMRegAnchor _;
  auto const fp = vmfp();
  auto const sk = SrcKey(fp->func(), vmpc(), resumeModeFromActRec(fp));

  auto profile = [&] {
    {
      ProfileMap::const_accessor it;
      if (s_profileMap.find(it, sk)) return it->second;
    }

    auto prof = std::make_unique<LoggingProfile>(sk);
    if (ad->isStatic()) {
      auto const size = sizeof(LoggingArray);
      auto lad = static_cast<LoggingArray*>(
          RO::EvalLowStaticArrays ? low_malloc(size) : uncounted_malloc(size));

      lad->initHeader_16(getBespokeKind(ad->kind()), StaticValue,
                         ad->isLegacyArray() ? ArrayData::kLegacyArray : 0);
      lad->setLayout(s_layout);
      lad->wrapped = ad;
      lad->profile = prof.get();

      prof->staticArray = lad;
    }

    ProfileMap::accessor insert;
    if (s_profileMap.insert(insert, sk)) {
      insert->second = prof.release();
      MemoryStats::LogAlloc(AllocKind::StaticArray, sizeof(LoggingArray));
    } else {
      // Someone beat us; clean up
      if (ad->isStatic()) {
        if (RO::EvalLowStaticArrays) {
          low_free(prof->staticArray);
        } else {
          uncounted_free(prof->staticArray);
        }
      }
    }

    return insert->second;
  }();

  auto const shouldEmitBespoke = [&] {
    if (shouldTestBespokeArrayLikes()) {
      FTRACE(5, "Observe rid: {}\n", requestCount());
      return !jit::mcgen::retranslateAllEnabled() || requestCount() % 2 == 1;
    } else {
      if (RO::EvalEmitLoggingArraySampleRate == 0) return false;

      auto const skCount = profile->sampleCount++;
      FTRACE(5, "Observe SrcKey count: {}\n", skCount);
      return (skCount - 1) % RO::EvalEmitLoggingArraySampleRate == 0;
    }
  }();

  if (shouldEmitBespoke) {
    FTRACE(5, "Emit bespoke at {}\n", sk.getSymbol());
    return ad->isStatic() ? profile->staticArray
                          : makeWithProfile(ad, profile);
  } else {
    FTRACE(5, "Emit vanilla at {}\n", sk.getSymbol());
    return ad;
  }
}

const ArrayData* maybeEnableLogging(const ArrayData* ad) {
  return maybeEnableLogging(const_cast<ArrayData*>(ad));
}

//////////////////////////////////////////////////////////////////////////////

bool LoggingArray::checkInvariants() const {
  assertx(!isVanilla());
  assertx(kindIsValid());
  assertx(wrapped->isVanilla());
  assertx(wrapped->kindIsValid());
  assertx(wrapped->toDataType() == toDataType());
  assertx(asBespoke(this)->layout() == s_layout);
  assertx(m_kind == getBespokeKind(wrapped->kind()));
  assertx(isLegacyArray() == wrapped->isLegacyArray());
  return true;
}

LoggingArray* LoggingArray::asLogging(ArrayData* ad) {
  auto const result = reinterpret_cast<LoggingArray*>(ad);
  result->checkInvariants();
  return result;
}
const LoggingArray* LoggingArray::asLogging(const ArrayData* ad) {
  return asLogging(const_cast<ArrayData*>(ad));
}

LoggingArray* LoggingArray::updateKind() {
  auto const kind = getBespokeKind(wrapped->kind());
  assertx(IMPLIES(kind != m_kind, hasExactlyOneRef()));
  m_kind = kind;
  assertx(checkInvariants());
  return this;
}

size_t LoggingLayout::heapSize(const ArrayData*) const {
  return sizeof(LoggingArray);
}
void LoggingLayout::scan(const ArrayData* ad, type_scan::Scanner& scan) const {
  scan.scan(LoggingArray::asLogging(ad)->wrapped);
}

ArrayData* LoggingLayout::escalateToVanilla(
    const ArrayData* ad, const char* /*reason*/) const {
  auto wrapped = LoggingArray::asLogging(ad)->wrapped;
  wrapped->incRefCount();
  return wrapped;
}

void LoggingLayout::convertToUncounted(
    ArrayData* ad, DataWalker::PointerMap* seen) const {
  auto lad = LoggingArray::asLogging(ad);
  auto tv = make_array_like_tv(lad->wrapped);
  ConvertTvToUncounted(&tv, seen);
  lad->wrapped = val(tv).parr;
}

void LoggingLayout::releaseUncounted(ArrayData* ad) const {
  auto tv = make_array_like_tv(LoggingArray::asLogging(ad)->wrapped);
  ReleaseUncountedTv(&tv);
}

//////////////////////////////////////////////////////////////////////////////

void LoggingLayout::release(ArrayData* ad) const {
  LoggingArray::asLogging(ad)->wrapped->decRefAndRelease();
  tl_heap->objFreeIndex(ad, kSizeIndex);
}
size_t LoggingLayout::size(const ArrayData* ad) const {
  return LoggingArray::asLogging(ad)->wrapped->size();
}
bool LoggingLayout::isVectorData(const ArrayData* ad) const {
  return LoggingArray::asLogging(ad)->wrapped->isVectorData();
}

TypedValue LoggingLayout::getInt(const ArrayData* ad, int64_t k) const {
  return LoggingArray::asLogging(ad)->wrapped->get(k);
}
TypedValue LoggingLayout::getStr(const ArrayData* ad, const StringData* k) const {
  return LoggingArray::asLogging(ad)->wrapped->get(k);
}
TypedValue LoggingLayout::getKey(const ArrayData* ad, ssize_t pos) const {
  return LoggingArray::asLogging(ad)->wrapped->nvGetKey(pos);
}
TypedValue LoggingLayout::getVal(const ArrayData* ad, ssize_t pos) const {
  return LoggingArray::asLogging(ad)->wrapped->nvGetVal(pos);
}
ssize_t LoggingLayout::getIntPos(const ArrayData* ad, int64_t k) const {
  return LoggingArray::asLogging(ad)->wrapped->nvGetIntPos(k);
}
ssize_t LoggingLayout::getStrPos(const ArrayData* ad, const StringData* k) const {
  return LoggingArray::asLogging(ad)->wrapped->nvGetStrPos(k);
}

namespace {

ArrayData* escalate(LoggingArray* lad, ArrayData* result) {
  if (result == lad->wrapped) return lad;
  return makeWithProfile(result, lad->profile);
}

arr_lval escalate(LoggingArray* lad, arr_lval result) {
  return arr_lval{escalate(lad, result.arr), result};
}

template <typename F>
decltype(auto) mutate(ArrayData* ad, F&& f) {
  auto lad = LoggingArray::asLogging(ad);
  auto const cow = lad->cowCheck();
  if (cow) lad->wrapped->incRefCount();
  SCOPE_EXIT { if (cow) lad->wrapped->decRefCount(); };
  return escalate(lad, f(lad->wrapped));
}

}

arr_lval LoggingLayout::lvalInt(ArrayData* ad, int64_t k) const {
  return mutate(ad, [&](ArrayData* arr) { return arr->lval(k); });
}
arr_lval LoggingLayout::lvalStr(ArrayData* ad, StringData* k) const {
  return mutate(ad, [&](ArrayData* arr) { return arr->lval(k); });
}
ArrayData* LoggingLayout::setInt(ArrayData* ad, int64_t k, TypedValue v) const {
  return mutate(ad, [&](ArrayData* w) { return w->set(k, v); });
}
ArrayData* LoggingLayout::setStr(ArrayData* ad, StringData* k, TypedValue v) const {
  return mutate(ad, [&](ArrayData* w) { return w->set(k, v); });
}
ArrayData* LoggingLayout::removeInt(ArrayData* ad, int64_t k) const {
  return mutate(ad, [&](ArrayData* w) { return w->remove(k); });
}
ArrayData* LoggingLayout::removeStr(ArrayData* ad, const StringData* k) const {
  return mutate(ad, [&](ArrayData* w) { return w->remove(k); });
}

ssize_t LoggingLayout::iterBegin(const ArrayData* ad) const {
  return LoggingArray::asLogging(ad)->wrapped->iter_begin();
}
ssize_t LoggingLayout::iterLast(const ArrayData* ad) const {
  return LoggingArray::asLogging(ad)->wrapped->iter_last();
}
ssize_t LoggingLayout::iterEnd(const ArrayData* ad) const {
  return LoggingArray::asLogging(ad)->wrapped->iter_end();
}
ssize_t LoggingLayout::iterAdvance(const ArrayData* ad, ssize_t prev) const {
  return LoggingArray::asLogging(ad)->wrapped->iter_advance(prev);
}
ssize_t LoggingLayout::iterRewind(const ArrayData* ad, ssize_t prev) const {
  return LoggingArray::asLogging(ad)->wrapped->iter_rewind(prev);
}

ArrayData* LoggingLayout::append(ArrayData* ad, TypedValue v) const {
  return mutate(ad, [&](ArrayData* w) { return w->append(v); });
}
ArrayData* LoggingLayout::prepend(ArrayData* ad, TypedValue v) const {
  return mutate(ad, [&](ArrayData* w) { return w->prepend(v); });
}
ArrayData* LoggingLayout::merge(ArrayData* ad, const ArrayData* arr) const {
  return mutate(ad, [&](ArrayData* w) { return w->merge(arr); });
}
ArrayData* LoggingLayout::pop(ArrayData* ad, Variant& ret) const {
  return mutate(ad, [&](ArrayData* w) { return w->pop(ret); });
}
ArrayData* LoggingLayout::dequeue(ArrayData* ad, Variant& ret) const {
  return mutate(ad, [&](ArrayData* w) { return w->dequeue(ret); });
}
ArrayData* LoggingLayout::renumber(ArrayData* ad) const {
  return mutate(ad, [&](ArrayData* w) { return w->renumber(); });
}

namespace {

template <typename F>
ArrayData* conv(ArrayData* ad, F&& f) {
  auto const lad = LoggingArray::asLogging(ad);
  auto const result = f(lad->wrapped);
  if (result == lad->wrapped) return lad->updateKind();
  return makeWithProfile(result, lad->profile);
}

}

ArrayData* LoggingLayout::copy(const ArrayData* ad) const {
  auto const lad = LoggingArray::asLogging(ad);
  return makeWithProfile(lad->wrapped->copy(), lad->profile);
}
ArrayData* LoggingLayout::toVArray(ArrayData* ad, bool copy) const {
  return conv(ad, [=](ArrayData* w) { return w->toVArray(copy); });
}
ArrayData* LoggingLayout::toDArray(ArrayData* ad, bool copy) const {
  return conv(ad, [=](ArrayData* w) { return w->toDArray(copy); });
}
ArrayData* LoggingLayout::toVec(ArrayData* ad, bool copy) const {
  return conv(ad, [=](ArrayData* w) { return w->toVec(copy); });
}
ArrayData* LoggingLayout::toDict(ArrayData* ad, bool copy) const {
  return conv(ad, [=](ArrayData* w) { return w->toDict(copy); });
}
ArrayData* LoggingLayout::toKeyset(ArrayData* ad, bool copy) const {
  return conv(ad, [=](ArrayData* w) { return w->toKeyset(copy); });
}

void LoggingLayout::setLegacyArrayInPlace(ArrayData* ad, bool legacy) const {
  assert(ad->hasExactlyOneRef());
  auto const lad = LoggingArray::asLogging(ad);
  if (lad->wrapped->cowCheck()) {
    auto const nad = lad->wrapped->copy();
    lad->wrapped->decRefCount();
    lad->wrapped = nad;
  }
  lad->wrapped->setLegacyArray(legacy);
}

//////////////////////////////////////////////////////////////////////////////

}}
