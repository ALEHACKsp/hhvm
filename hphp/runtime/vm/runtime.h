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
#pragma once

#include "hphp/runtime/ext/generator/ext_generator.h"
#include "hphp/runtime/ext/asio/ext_async-function-wait-handle.h"
#include "hphp/runtime/ext/asio/ext_async-generator.h"
#include "hphp/runtime/ext/std/ext_std_errorfunc.h"
#include "hphp/runtime/vm/act-rec.h"
#include "hphp/runtime/vm/event-hook.h"
#include "hphp/runtime/vm/func.h"
#include "hphp/runtime/vm/reified-generics.h"
#include "hphp/runtime/vm/resumable.h"
#include "hphp/runtime/base/builtin-functions.h"
#include "hphp/runtime/base/execution-context.h"
#include "hphp/runtime/base/object-data.h"
#include "hphp/runtime/base/stats.h"
#include "hphp/runtime/base/tv-refcount.h"
#include "hphp/runtime/base/tv-val.h"

namespace HPHP {

struct HhbcExtFuncInfo;
struct HhbcExtClassInfo;

StringData* concat_is(int64_t v1, StringData* v2);
StringData* concat_si(StringData* v1, int64_t v2);
StringData* concat_ss(StringData* v1, StringData* v2);
StringData* concat_s3(StringData* v1, StringData* v2, StringData* v3);
StringData* concat_s4(StringData* v1, StringData* v2,
                      StringData* v3, StringData* v4);

void print_string(StringData* s);
void print_int(int64_t i);
void print_boolean(bool val);

void raiseWarning(const StringData* sd);
void raiseNotice(const StringData* sd);
[[noreturn]] void throwArrayIndexException(const ArrayData *ad, int64_t index);
[[noreturn]] void throwArrayKeyException(const ArrayData *ad, const StringData* key);
std::string formatParamInOutMismatch(const char* fname, uint32_t index,
                                   bool funcByRef);
[[noreturn]] void throwParamInOutMismatch(const Func* func, uint32_t index);
[[noreturn]] void throwParamInOutMismatchRange(const Func* func,
                                               unsigned firstBit, uint64_t mask,
                                               uint64_t vals);
[[noreturn]] void throwInvalidUnpackArgs();
void raiseRxCallViolation(const ActRec* caller, const Func* callee);

inline Iter*
frame_iter(const ActRec* fp, int i) {
  return (Iter*)(uintptr_t(fp)
          - uintptr_t(fp->func()->numLocals() * sizeof(TypedValue))
          - uintptr_t((i+1) * sizeof(Iter)));
}

inline TypedValue*
frame_local(const ActRec* fp, int n) {
  return (TypedValue*)(uintptr_t(fp) - uintptr_t((n+1) * sizeof(TypedValue)));
}

inline Resumable*
frame_resumable(const ActRec* fp) {
  assertx(isResumed(fp));
  return (Resumable*)((char*)fp - Resumable::arOff());
}

inline c_AsyncFunctionWaitHandle*
frame_afwh(const ActRec* fp) {
  assertx(fp->func()->isAsyncFunction());
  auto resumable = frame_resumable(fp);
  auto arOffset = c_AsyncFunctionWaitHandle::arOff();
  auto waitHandle = (c_AsyncFunctionWaitHandle*)((char*)resumable - arOffset);
  assertx(waitHandle->getVMClass() == c_AsyncFunctionWaitHandle::classof());
  return waitHandle;
}

inline Generator*
frame_generator(const ActRec* fp) {
  assertx(fp->func()->isNonAsyncGenerator());
  auto resumable = frame_resumable(fp);
  return (Generator*)((char*)resumable - Generator::resumableOff());
}

inline AsyncGenerator*
frame_async_generator(const ActRec* fp) {
  assertx(fp->func()->isAsyncGenerator());
  auto resumable = frame_resumable(fp);
  return (AsyncGenerator*)((char*)resumable -
    AsyncGenerator::resumableOff());
}

void ALWAYS_INLINE
frame_free_locals_helper_inl(ActRec* fp, int numLocals) {
  assertx(numLocals == fp->func()->numLocals());
  // Free locals
  for (int i = numLocals - 1; i >= 0; --i) {
    TRACE_MOD(Trace::runtime, 5,
              "RetC: freeing %d'th local of %d\n", i,
              fp->func()->numLocals());
    tvDecRefGen(*frame_local(fp, i));
  }
}

void ALWAYS_INLINE
frame_free_locals_inl_no_hook(ActRec* fp, int numLocals) {
  frame_free_locals_helper_inl(fp, numLocals);
  if (fp->func()->cls() && fp->hasThis()) {
    decRefObj(fp->getThis());
  }
}

void ALWAYS_INLINE
frame_free_locals_inl(ActRec* fp, int numLocals, TypedValue* rv) {
  frame_free_locals_inl_no_hook(fp, numLocals);
  EventHook::FunctionReturn(fp, *rv);
}

void ALWAYS_INLINE
frame_free_locals_no_this_inl(ActRec* fp, int numLocals, TypedValue* rv) {
  frame_free_locals_helper_inl(fp, numLocals);
  EventHook::FunctionReturn(fp, *rv);
}

// Helper for iopFCallBuiltin.
void ALWAYS_INLINE
frame_free_args(TypedValue* args, int count) {
  for (auto i = count; i--; ) tvDecRefGen(*(args - i));
}

int64_t zero_error_level();
void restore_error_level(int64_t oldLevel);

}
