<?hh
// Copyright (c) Facebook, Inc. and its affiliates. All Rights Reserved.

<<__InferFlows>>
function casting(num $x): int {
  if ($x as int) {
    return $x;
  } else {
    return 0;
  }
}

<<__InferFlows>>
function variable_in_scope(): int {
  do {
    // $x is always in scope
    $x = 0;
  } while (false);

  // IFC never registers $x, but the analysis does not fatal
  return $x;
}
