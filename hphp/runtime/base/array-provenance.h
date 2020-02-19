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

#ifndef incl_HPHP_ARRAY_PROVENANCE_H
#define incl_HPHP_ARRAY_PROVENANCE_H

#include "hphp/runtime/base/runtime-option.h"
#include "hphp/runtime/base/typed-value.h"
#include "hphp/runtime/base/types.h"

#include "hphp/util/low-ptr.h"
#include "hphp/util/rds-local.h"

#include <folly/Format.h>
#include <folly/Optional.h>

namespace HPHP {

struct APCArray;
struct ArrayData;
struct StringData;
struct c_WaitableWaitHandle;
struct AsioExternalThreadEvent;

namespace arrprov {

///////////////////////////////////////////////////////////////////////////////

/*
 * A provenance annotation
 *
 * We need to store the filename and line since when assembling units, we
 * don't necessarily have the final Unit allocated yet. It may be faster to
 * make this a tagged union or store a different Tag type for static arrays
 */
struct Tag {
  constexpr Tag() = default;
  constexpr Tag(const StringData* filename, int line)
    : m_filename(filename)
    , m_line(line)
  {}

  const StringData* filename() const { return m_filename; }
  int line() const { return m_line; }

  bool operator==(const Tag& other) const {
    return m_filename == other.m_filename &&
           m_line == other.m_line;
  }
  bool operator!=(const Tag& other) const { return !(*this == other); }

  std::string toString() const;

private:
  const StringData* m_filename{nullptr};
  int m_line{0};
};

/*
 * This is a separate struct so it can live in RDS and not be GC scanned--the
 * actual RDS-local handle is kept in the implementation
 */
struct ArrayProvenanceTable {
  /* The table itself -- allocated in general heap */
  folly::F14FastMap<const void*, Tag> tags;

  /*
   * We never dereference ArrayData*s from this table--so it's safe for the GC
   * to ignore them in this table
   */
  TYPE_SCAN_IGNORE_FIELD(tags);
};

///////////////////////////////////////////////////////////////////////////////

/*
 * Create a tag based on the current PC and unit.
 *
 * Attempts to sync VM regs and returns folly::none on failure.
 */
folly::Optional<Tag> tagFromPC();

/*
 * RAII struct for modifying the behavior of tagFromPC().
 *
 * When this is in effect we use the tag provided instead of computing a
 * backtrace
 */
struct TagOverride {
  explicit TagOverride(Tag tag);
  ~TagOverride();

  TagOverride(TagOverride&&) = delete;
  TagOverride(const TagOverride&) = delete;

  TagOverride& operator=(const TagOverride&) = delete;
  TagOverride& operator=(TagOverride&&) = delete;

private:
  folly::Optional<Tag> m_saved_tag;
};

/*
 * Whether `a` admits a provenance tag.
 *
 * Depends on the ArrProv.* runtime options.
 */
bool arrayWantsTag(const ArrayData* a);
bool arrayWantsTag(const APCArray* a);
bool arrayWantsTag(const AsioExternalThreadEvent* a);

/*
 * Get the provenance tag for `a`.
 */
folly::Optional<Tag> getTag(const ArrayData* a);
folly::Optional<Tag> getTag(const APCArray* a);
folly::Optional<Tag> getTag(const AsioExternalThreadEvent* ev);

/*
 * Set mode: insert or emplace.
 *
 * Just controls whether we assert about provenance not already being set: we
 * assert for Insert mode, and not for Emplace.
 */
enum class Mode { Insert, Emplace };

/*
 * Set the provenance tag for `a` to `tag`.
 */
template<Mode mode = Mode::Insert> void setTag(ArrayData* a, Tag tag);
template<Mode mode = Mode::Insert> void setTag(const APCArray* a, Tag tag);
template<Mode mode = Mode::Insert> void setTag(AsioExternalThreadEvent* ev, Tag tag);

/*
 * Clear a tag for a released array---only call this if the array is henceforth
 * unreachable or no longer of a kind that accepts provenance tags
 */
void clearTag(ArrayData* ad);
void clearTag(const APCArray* a);
void clearTag(AsioExternalThreadEvent* ev);

/*
 * Invalidates the old tag on the provided array and reassigns one from the
 * current PC, if the array still admits a tag.
 *
 * If the array no longer admits a tag, but has one set, clears it.
 *
 */
void reassignTag(ArrayData* ad);

/*
 * Produce a static array with the given provenance tag.
 *
 * If no tag is provided, we attempt to make one from vmpc(), and failing that
 * we just return the input array.
 */
ArrayData* tagStaticArr(ArrayData* ad, folly::Optional<Tag> tag = folly::none);

///////////////////////////////////////////////////////////////////////////////

/*
 * Recursively tag the given TypedValue, tagging it (if necessary), and if it is
 * an array-like, recursively tagging of its values (if necessary).
 *
 * This function will tag values within, say, a dict, even if it doesn't tag the
 * dict itself. This behavior is important because it allows us to implement
 * provenance for (nested) static arrays in ProvenanceSkipFrame functions.
 *
 * The only other type that can contain nested arrays are objects. This function
 * does NOT tag through objects; instead, it raises notices that it found them.
 * (It will emit at most one notice per call.)
 *
 * This method will return a new TypedValue or modify and inc-ref `in`.
 */
TypedValue tagTvRecursively(TypedValue in);

/*
 * Recursively mark the given TV as being a legacy array. This function has the
 * same recursive behavior as tagTvRecursively, except that in addition to
 * raising a notice on encountering an object, it will also raise (up to one)
 * notice on encountering a vec or dict.
 *
 * The extra notice is needed because we won't be able to distinguish between
 * vecs and varrays, or between dicts and darrays, post the HAM flag flip.
 *
 * This method will return a new TypedValue or modify and inc-ref `in`.
 */
TypedValue markTvRecursively(TypedValue in);

/*
 * Mark the given TV as being a legacy array.
 *
 * This method will return a new TypedValue or modify and inc-ref `in`.
 */
TypedValue markTvShallow(TypedValue in);

///////////////////////////////////////////////////////////////////////////////

}}

#endif
