<?hh

class A {
  public function foo() { }
  public $bar0 = 0;
  public $bar1 = 1;
}
<<__EntryPoint>> function main(): void {
$a = new A;
$b = darray[ 0 => 'A', 1 => 'B' ];
list ($a->bar0,  $a->bar1) = $b;
list ($a->foo(), $a->bar1) = $b;
}
