<?hh

final record Foo {
  x: int,
  y: string,
}
<<__EntryPoint>> function main(): void {
$foo = Foo['x' => 1];
}
