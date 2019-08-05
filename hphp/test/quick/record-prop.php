<?hh

class Foo {
  public A $x;
}

final record A {
  x: int,
}

final record B {
  x: int,
}
<<__EntryPoint>> function main(): void {
$a = A['x' => 1];
$b = B['x' => 2];

$foo = new Foo();
$foo->x = $a;
var_dump($foo->x['x']);

$foo->x = $b;
var_dump($foo->x['x']);
}
