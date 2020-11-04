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

#include "hphp/runtime/base/bespoke-layout.h"

#include "hphp/runtime/base/bespoke/layout.h"
#include "hphp/runtime/base/bespoke/logging-array.h"
#include "hphp/runtime/base/bespoke/bespoke-top.h"
#include "hphp/runtime/base/bespoke-array.h"
#include "hphp/runtime/vm/jit/ssa-tmp.h"

namespace HPHP {

using namespace jit;
using namespace jit::irgen;

BespokeLayout::BespokeLayout(const bespoke::Layout* layout)
  : m_layout(layout)
{
  assertx(layout);
}

BespokeLayout::BespokeLayout(const bespoke::ConcreteLayout* layout)
  : m_layout(layout)
{
  assertx(layout);
}

BespokeLayout BespokeLayout::FromIndex(uint16_t index) {
  return BespokeLayout{bespoke::Layout::FromIndex({index})};
}

BespokeLayout BespokeLayout::LoggingLayout() {
  return BespokeLayout::FromIndex(bespoke::LoggingArray::GetLayoutIndex().raw);
}

BespokeLayout BespokeLayout::TopLayout() {
  return BespokeLayout::FromIndex(bespoke::BespokeTop::GetLayoutIndex().raw);
}

uint16_t BespokeLayout::index() const {
  return m_layout->index().raw;
}

const std::string& BespokeLayout::describe() const {
  return m_layout->describe();
}

namespace {
bool checkLayoutMatches(const bespoke::Layout* layout, SSATmp* arr) {
  auto const DEBUG_ONLY layoutType =
    arr->type().unspecialize().narrowToBespokeLayout(BespokeLayout(layout));
  // TODO(mcolavita): Once we have a type hierarchy, we don't need IMPLIES
  assertx(IMPLIES(layout->isConcrete(), arr->type() <= layoutType));

  return true;
}
}

SSATmp* BespokeLayout::emitGet(
    IRGS& env, SSATmp* arr, SSATmp* key, Block* taken) const {
  assertx(checkLayoutMatches(m_layout, arr));
  return m_layout->emitGet(env, arr, key, taken);
}

SSATmp* BespokeLayout::emitElem(
    IRGS& env, SSATmp* arr, SSATmp* key, bool throwOnMissing) const {
  assertx(checkLayoutMatches(m_layout, arr));
  return m_layout->emitElem(env, arr, key, throwOnMissing);
}

SSATmp* BespokeLayout::emitSet(
    IRGS& env, SSATmp* arr, SSATmp* key, SSATmp* val) const {
  assertx(checkLayoutMatches(m_layout, arr));
  return m_layout->emitSet(env, arr, key, val);
}

SSATmp* BespokeLayout::emitAppend(IRGS& env, SSATmp* arr, SSATmp* val) const {
  assertx(checkLayoutMatches(m_layout, arr));
  return m_layout->emitAppend(env, arr, val);
}

SSATmp* BespokeLayout::emitEscalateToVanilla(IRGS& env, SSATmp* arr,
                                             const char* reason) const {
  assertx(checkLayoutMatches(m_layout, arr));
  return m_layout->emitEscalateToVanilla(env, arr, reason);
}

SSATmp* BespokeLayout::emitIterFirstPos(IRGS& env, SSATmp* arr) const {
  assertx(checkLayoutMatches(m_layout, arr));
  return m_layout->emitIterFirstPos(env, arr);
}

SSATmp* BespokeLayout::emitIterLastPos(IRGS& env, SSATmp* arr) const {
  assertx(checkLayoutMatches(m_layout, arr));
  return m_layout->emitIterLastPos(env, arr);
}

SSATmp* BespokeLayout::emitIterPos(IRGS& env, SSATmp* arr, SSATmp* idx) const {
  assertx(checkLayoutMatches(m_layout, arr));
  return m_layout->emitIterPos(env, arr, idx);
}

SSATmp* BespokeLayout::emitIterElm(IRGS& env, SSATmp* arr, SSATmp* pos) const {
  assertx(checkLayoutMatches(m_layout, arr));
  return m_layout->emitIterElm(env, arr, pos);
}

SSATmp* BespokeLayout::emitIterGetKey(
    IRGS& env, SSATmp* arr, SSATmp* elm) const {
  assertx(checkLayoutMatches(m_layout, arr));
  return m_layout->emitIterGetKey(env, arr, elm);
}

SSATmp* BespokeLayout::emitIterGetVal(
    IRGS& env, SSATmp* arr, SSATmp* elm) const {
  assertx(checkLayoutMatches(m_layout, arr));
  return m_layout->emitIterGetVal(env, arr, elm);
}

}

