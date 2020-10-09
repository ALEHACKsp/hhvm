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

#include <sys/types.h>

namespace HPHP {

//////////////////////////////////////////////////////////////////////

struct ArrayData;
struct ObjectData;

//////////////////////////////////////////////////////////////////////

ArrayData* castObjToVec(ObjectData*);
ArrayData* castObjToDict(ObjectData*);
ArrayData* castObjToKeyset(ObjectData*);
ArrayData* castObjToVArray(ObjectData*);
ArrayData* castObjToDArray(ObjectData*);

//////////////////////////////////////////////////////////////////////

}

