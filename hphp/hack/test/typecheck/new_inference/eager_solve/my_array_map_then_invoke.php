<?hh // strict
// Copyright 2004-present Facebook. All Rights Reserved.

class C {
  public function foo():void { }
}

/* HH_FIXME[4110] */
function my_array_map<T1,T2>((function(T1):T2) $f, vec<T1> $v): vec<T2> {
}

function testit():void {
  $x = vec[new C()];
  $y = my_array_map($c ==> $c, $x);
  $y[0]->foo();
}
