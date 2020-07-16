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

#ifndef incl_HPHP_BUILTIN_FUNCTIONS_H_
#define incl_HPHP_BUILTIN_FUNCTIONS_H_

#include "hphp/runtime/base/type-variant.h"
#include "hphp/runtime/base/variable-unserializer.h"
#include "hphp/runtime/vm/bytecode.h"
#include "hphp/util/functional.h"
#include "hphp/util/portability.h"
#include "hphp/runtime/base/req-root.h"

namespace HPHP {
///////////////////////////////////////////////////////////////////////////////
extern const StaticString s_self;
extern const StaticString s_parent;
extern const StaticString s_static;

extern const StaticString s_cmpWithCollection;
extern const StaticString s_cmpWithVec;
extern const StaticString s_cmpWithDict;
extern const StaticString s_cmpWithKeyset;
extern const StaticString s_cmpWithClsMeth;
extern const StaticString s_cmpWithRClsMeth;
extern const StaticString s_cmpWithRFunc;
extern const StaticString s_cmpWithRecord;
extern const StaticString s_cmpWithNonArr;
extern const StaticString s_cmpWithFunc;

///////////////////////////////////////////////////////////////////////////////
// operators

inline String concat(const String& s1, const String& s2) {
  return s1 + s2;
}

String concat3(const String& s1, const String& s2, const String& s3);
String concat4(const String& s1, const String& s2, const String& s3,
               const String& s4);

///////////////////////////////////////////////////////////////////////////////

[[noreturn]] void NEVER_INLINE throw_missing_this(const Func* f);
[[noreturn]] void NEVER_INLINE throw_has_this_need_static(const Func* f);
void NEVER_INLINE throw_invalid_property_name(const String& name);

[[noreturn]]
void NEVER_INLINE throw_call_reified_func_without_generics(const Func* f);

[[noreturn]] void NEVER_INLINE throw_implicit_context_exception(std::string);

[[noreturn]]
void throw_exception(const Object& e);

///////////////////////////////////////////////////////////////////////////////
// type testing

inline bool is_null(tv_rval c) {
  assertx(tvIsPlausible(*c));
  return tvIsNull(c);
}

inline bool is_bool(const TypedValue* c) {
  assertx(tvIsPlausible(*c));
  return tvIsBool(c);
}

inline bool is_int(const TypedValue* c) {
  assertx(tvIsPlausible(*c));
  return tvIsInt(c);
}

inline bool is_double(const TypedValue* c) {
  assertx(tvIsPlausible(*c));
  return tvIsDouble(c);
}

inline bool is_string(const TypedValue* c) {
  if (tvIsString(c)) return true;
  if (tvIsFunc(c)) {
    if (RuntimeOption::EvalIsStringNotices) {
      raise_notice("Func used in is_string");
    }
    return true;
  }
  if (tvIsClass(c)) {
    if (RuntimeOption::EvalIsStringNotices) {
      raise_notice("Class used in is_string");
    }
    return true;
  }
  return false;
}

// This function behaves how most callers of raise_array_serialization_notice
// should behave: it checks if `tv` *should* have a provenance tag and then
// logs a serialization notice of some kind if so.
//
// If we trace through call sites of the bare function, we'll find a number of
// places where we're incorrectly losing provenance logs. Clean this up soon.
inline void maybe_raise_array_serialization_notice(
    SerializationSite site, const TypedValue* tv) {
  assertx(isArrayLikeType(tv->m_type));
  auto const ad = tv->m_data.parr;
  if (arrprov::arrayWantsTag(ad)) {
    raise_array_serialization_notice(site, ad);
  }
}

inline bool is_any_array(const TypedValue* c, bool logOnHackArrays) {
  assertx(tvIsPlausible(*c));
  if (tvIsClsMeth(c) && RO::EvalIsCompatibleClsMethType) {
    if (RO::EvalIsVecNotices) {
      raise_notice(Strings::CLSMETH_COMPAT_IS_ANY_ARR);
    }
    return true;
  }

  if (logOnHackArrays && RO::EvalWidenIsArrayLogs) {
    if (tvIsVec(c)) {
      raise_hackarr_compat_notice(Strings::HACKARR_COMPAT_VEC_IS_ARR);
    } else if (tvIsDict(c)) {
      raise_hackarr_compat_notice(Strings::HACKARR_COMPAT_DICT_IS_ARR);
    } else if (tvIsKeyset(c)) {
      raise_hackarr_compat_notice(Strings::HACKARR_COMPAT_KEYSET_IS_ARR);
    }
  }
  return tvIsArrayLike(c);
}

inline bool is_array(const TypedValue* c, bool logOnHackArrays) {
  assertx(tvIsPlausible(*c));

  if (tvIsArray(c)) {
    maybe_raise_array_serialization_notice(SerializationSite::IsArray, c);
    return true;
  }

  if (tvIsClsMeth(c)) {
    if (!RO::EvalHackArrDVArrs && RO::EvalIsCompatibleClsMethType) {
      if (RO::EvalIsVecNotices) raise_notice(Strings::CLSMETH_COMPAT_IS_ARR);
      return true;
    }
    return false;
  }

  auto const hacLogging = [&](const char* msg) {
    if (RO::EvalHackArrCompatIsArrayNotices) raise_hackarr_compat_notice(msg);
  };
  if (logOnHackArrays /* let's get rid of this condition if we can */) {
    if (tvIsVec(c)) {
      hacLogging(Strings::HACKARR_COMPAT_VEC_IS_ARR);
      maybe_raise_array_serialization_notice(SerializationSite::IsArray, c);
    } else if (tvIsDict(c)) {
      hacLogging(Strings::HACKARR_COMPAT_DICT_IS_ARR);
      maybe_raise_array_serialization_notice(SerializationSite::IsArray, c);
    } else if (tvIsKeyset(c)) {
      hacLogging(Strings::HACKARR_COMPAT_KEYSET_IS_ARR);
      assertx(!arrprov::arrayWantsTag(c->m_data.parr));
    }
  }
  return false;
}

inline bool is_vec(const TypedValue* c) {
  assertx(tvIsPlausible(*c));

  if (tvIsVec(c)) {
    maybe_raise_array_serialization_notice(SerializationSite::IsVec, c);
    return true;
  }

  auto const hacLogging = [&](const char* msg) {
    if (RO::EvalHackArrCompatIsVecDictNotices) raise_hackarr_compat_notice(msg);
  };
  if (tvIsClsMeth(c)) {
    if (RO::EvalHackArrDVArrs && RO::EvalIsCompatibleClsMethType) {
      if (RO::EvalIsVecNotices) raise_notice(Strings::CLSMETH_COMPAT_IS_VEC);
      return true;
    }

    if (!RO::EvalHackArrDVArrs) {
      hacLogging(Strings::HACKARR_COMPAT_VARR_IS_VEC);
    }
    return false;
  }

  if (tvIsArrayLike(c) && c->m_data.parr->isVArray()) {
    hacLogging(Strings::HACKARR_COMPAT_VARR_IS_VEC);
    maybe_raise_array_serialization_notice(SerializationSite::IsVec, c);
  }
  return false;
}

inline bool is_dict(const TypedValue* c) {
  assertx(tvIsPlausible(*c));

  if (tvIsDict(c)) {
    maybe_raise_array_serialization_notice(SerializationSite::IsDict, c);
    return true;
  }

  auto const hacLogging = [&](const char* msg) {
    if (RO::EvalHackArrCompatIsVecDictNotices) raise_hackarr_compat_notice(msg);
  };
  if (tvIsArrayLike(c) && c->m_data.parr->isDArray()) {
    hacLogging(Strings::HACKARR_COMPAT_DARR_IS_DICT);
    maybe_raise_array_serialization_notice(SerializationSite::IsDict, c);
  }
  return false;
}

inline bool is_keyset(const TypedValue* c) {
  assertx(tvIsPlausible(*c));
  return tvIsKeyset(c);
}

inline bool is_varray(const TypedValue* c) {
  assertx(tvIsPlausible(*c));

  // Is this line safe? It returns the correct result, but if it logs a notice,
  // it'll be for is_vec, not is_varray. That may be fine, post-HAM, because
  // only dynamic calls to is_varray will remain at that point.
  if (RuntimeOption::EvalHackArrDVArrs) return is_vec(c);

  if (tvIsArrayLike(c) && c->m_data.parr->isVArray()) {
    maybe_raise_array_serialization_notice(SerializationSite::IsVArray, c);
    return true;
  }

  if (tvIsClsMeth(c) && RO::EvalIsCompatibleClsMethType) {
    if (RO::EvalIsVecNotices) raise_notice(Strings::CLSMETH_COMPAT_IS_VARR);
    return true;
  }

  auto const hacLogging = [&](const char* msg) {
    if (RO::EvalHackArrCompatIsVecDictNotices) raise_hackarr_compat_notice(msg);
  };
  if (tvIsVec(c)) {
    hacLogging(Strings::HACKARR_COMPAT_VEC_IS_VARR);
    maybe_raise_array_serialization_notice(SerializationSite::IsVArray, c);
  }
  return false;
}

inline bool is_vec_or_varray(const TypedValue* c) {
  assertx(tvIsPlausible(*c));

  if (tvIsVec(c) || (tvIsArrayLike(c) && c->m_data.parr->isVArray())) {
    return true;
  }

  if (tvIsClsMeth(c) && RO::EvalIsCompatibleClsMethType) {
    if (RO::EvalIsVecNotices) {
      raise_notice(Strings::CLSMETH_COMPAT_IS_VEC_OR_VARR);
    }
    return true;
  }

  return false;
}

inline bool is_darray(const TypedValue* c) {
  assertx(tvIsPlausible(*c));

  // Is this line safe? It returns the correct result, but if it logs a notice,
  // it'll be for is_dict, not is_darray. That may be fine, post-HAM, because
  // only dynamic calls to is_darray will remain at that point.
  if (RuntimeOption::EvalHackArrDVArrs) return is_dict(c);

  if (tvIsArrayLike(c) && c->m_data.parr->isDArray()) {
    maybe_raise_array_serialization_notice(SerializationSite::IsDArray, c);
    return true;
  }

  auto const hacLogging = [&](const char* msg) {
    if (RO::EvalHackArrCompatIsVecDictNotices) raise_hackarr_compat_notice(msg);
  };
  if (tvIsDict(c)) {
    hacLogging(Strings::HACKARR_COMPAT_DICT_IS_DARR);
    maybe_raise_array_serialization_notice(SerializationSite::IsDArray, c);
  }
  return false;
}

inline bool is_dict_or_darray(const TypedValue* c) {
  assertx(tvIsPlausible(*c));

  if (tvIsDict(c) || (tvIsArrayLike(c) && c->m_data.parr->isDArray())) {
    return true;
  }

  return false;
}

inline bool is_object(const TypedValue* c) {
  assertx(tvIsPlausible(*c));
  return tvIsObject(c) &&
    c->m_data.pobj->getVMClass() != SystemLib::s___PHP_Incomplete_ClassClass;
}

inline bool is_clsmeth(const TypedValue* c) {
  assertx(tvIsPlausible(*c));
  return tvIsClsMeth(c);
}

inline bool is_fun(const TypedValue* c) {
  assertx(tvIsPlausible(*c));
  return tvIsFunc(c);
}

inline bool is_empty_string(const TypedValue* c) {
  return tvIsString(c) && c->m_data.pstr->empty();
}

///////////////////////////////////////////////////////////////////////////////
// misc functions

/*
 * Semantics of is_callable defined here:
 * http://php.net/manual/en/function.is-callable.php
 */
bool is_callable(const Variant& v, bool syntax_only, Variant* name);
/*
 * Equivalent to is_callable(v, false, nullptr)
 */
bool is_callable(const Variant& v);
bool array_is_valid_callback(const Array& arr);

enum class DecodeFlags { Warn, NoWarn, LookupOnly };
const HPHP::Func*
vm_decode_function(const_variant_ref function,
                   ActRec* ar,
                   ObjectData*& this_,
                   HPHP::Class*& cls,
                   bool& dynamic,
                   DecodeFlags flags = DecodeFlags::Warn,
                   bool genericsAlreadyGiven = false);

inline void
vm_decode_function(const_variant_ref function,
                   CallCtx& ctx,
                   DecodeFlags flags = DecodeFlags::Warn,
                   bool genericsAlreadyGiven = false) {
  ctx.func = vm_decode_function(function, nullptr, ctx.this_, ctx.cls,
                                ctx.dynamic, flags, genericsAlreadyGiven);
}

bool checkMethCallerTarget(const Func* meth, const Class* ctx, bool error);
void checkMethCaller(const Func* func, const Class* ctx);

Variant vm_call_user_func(const_variant_ref function, const Variant& params,
                          bool checkRef = false,
                          bool allowDynCallNoPointer = false);
template<typename T>
Variant vm_call_user_func(T&& t, const Variant& params, bool checkRef = false,
                          bool allowDynCallNoPointer = false) {
  const Variant function{std::forward<T>(t)};
  return vm_call_user_func(
    const_variant_ref{function}, params, checkRef, allowDynCallNoPointer
  );
}

// Invoke an arbitrary user-defined function.
// If you're considering calling this function for some new code, don't.
Variant invoke(const String& function, const Variant& params,
               bool allowDynCallNoPointer = false);

Variant invoke_static_method(const String& s, const String& method,
                             const Variant& params, bool fatal = true);

Variant o_invoke_failed(const char *cls, const char *meth,
                        bool fatal = true);

bool is_constructor_name(const char* func);
[[noreturn]] void throw_instance_method_fatal(const char *name);

[[noreturn]] void throw_invalid_collection_parameter();
[[noreturn]] void throw_invalid_operation_exception(StringData*);
[[noreturn]] void throw_division_by_zero_exception();
[[noreturn]] void throw_iterator_not_valid();
[[noreturn]] void throw_collection_property_exception();
[[noreturn]] void throw_collection_compare_exception();
[[noreturn]] void throw_varray_compare_exception();
[[noreturn]] void throw_darray_compare_exception();
[[noreturn]] void throw_vec_compare_exception();
[[noreturn]] void throw_dict_compare_exception();
[[noreturn]] void throw_keyset_compare_exception();
[[noreturn]] void throw_clsmeth_compare_exception();
[[noreturn]] void throw_rclsmeth_compare_exception();
[[noreturn]] void throw_record_compare_exception();
[[noreturn]] void throw_rfunc_compare_exception();
[[noreturn]] void throw_rec_non_rec_compare_exception();
[[noreturn]] void throw_arr_non_arr_compare_exception();
[[noreturn]] void throw_func_compare_exception();
[[noreturn]] void throw_param_is_not_container();
[[noreturn]] void throw_invalid_inout_base();
[[noreturn]] void throw_cannot_modify_immutable_object(const char* className);
[[noreturn]] void throw_cannot_modify_const_object(const char* className);
[[noreturn]] void throw_object_forbids_dynamic_props(const char* className);
[[noreturn]] void throw_cannot_modify_const_prop(const char* className,
                                                 const char* propName);
[[noreturn]] void throw_cannot_modify_static_const_prop(const char* className,
                                                        const char* propName);
[[noreturn]] void throw_late_init_prop(const Class* cls,
                                       const StringData* propName,
                                       bool isSProp);
[[noreturn]] void throw_parameter_wrong_type(TypedValue tv,
                                             const Func* callee,
                                             unsigned int arg_num,
                                             const StringData* type);

void check_collection_cast_to_array();

Object create_object_only(const String& s);
Object create_object(const String& s, const Array &params, bool init = true);
Object init_object(const String& s, const Array &params, ObjectData* o);

[[noreturn]] void throw_object(const Object& e);
#if ((__GNUC__ != 4) || (__GNUC_MINOR__ != 8))
// gcc-4.8 has a bug that causes incorrect code if we
// define this function.
[[noreturn]] void throw_object(Object&& e);
#endif

[[noreturn]] inline
void throw_object(const String& s, const Array& params, bool init = true) {
  throw_object(create_object(s, params, init));
}

void throw_missing_arguments_nr(const char *fn, int expected, int got)

  __attribute__((__cold__));

/**
 * Handler for exceptions thrown from user functions that we don't
 * allow exception propagation from.  E.g., object destructors or
 * certain callback hooks (user profiler). Implemented in
 * program-functions.cpp.
 */
void handle_destructor_exception(const char* situation = "Destructor");

/*
 * Deprecated wrappers for raising certain types of warnings.
 *
 * Don't use in new code.
 */
void raise_bad_type_warning(ATTRIBUTE_PRINTF_STRING const char *fmt, ...)
  ATTRIBUTE_PRINTF(1,2);
void raise_expected_array_warning(const char* fn = nullptr);
void raise_expected_array_or_collection_warning(const char* fn = nullptr);
void raise_invalid_argument_warning(ATTRIBUTE_PRINTF_STRING const char *fmt, ...)
  ATTRIBUTE_PRINTF(1,2) __attribute__((__cold__));

/**
 * Unsetting ClassName::StaticProperty.
 */
Variant throw_fatal_unset_static_property(const char *s, const char *prop);

// unserializable default value arguments such as TimeStamp::Current()
// are serialized as "\x01"
char const kUnserializableString[] = "\x01";

/**
 * Serialize/unserialize a variant into/from a string. We need these
 * two functions in runtime/base, as there are functions in
 * runtime/base that depend on these two functions.
 */
String f_serialize(const Variant& value);
String serialize_keep_dvarrays(const Variant& value);
Variant unserialize_ex(const String& str,
                       VariableUnserializer::Type type,
                       const Array& options = null_array);
Variant unserialize_ex(const char* str, int len,
                       VariableUnserializer::Type type,
                       const Array& options = null_array);

inline Variant unserialize_from_buffer(const char* str, int len,
                                       VariableUnserializer::Type type,
                                       const Array& options = null_array) {
  return unserialize_ex(str, len, type, options);
}

inline Variant unserialize_from_string(const String& str,
                                       VariableUnserializer::Type type,
                                       const Array& options = null_array) {
  return unserialize_from_buffer(str.data(), str.size(), type, options);
}

String resolve_include(const String& file, const char* currentDir,
                       bool (*tryFile)(const String& file, void* ctx),
                       void* ctx);
Variant include_impl_invoke(const String& file, bool once = false,
                            const char *currentDir = "",
                            bool callByHPHPInvoke = false);
Variant require(const String& file, bool once, const char* currentDir,
                bool raiseNotice);

bool function_exists(const String& function_name);

///////////////////////////////////////////////////////////////////////////////
}

#endif
