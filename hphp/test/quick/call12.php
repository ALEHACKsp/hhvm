<?hh
class A {
  public function __call($fn, $args) {
    var_dump($fn, $args);
    echo "\n\n";
  }
}
class B {
  public function test() {
    A::foo();
  }
}
class C extends B {
}

<<__EntryPoint>> function main(): void {
  $obj = new C;
  $obj->test();
}
