<?hh

// taking references
class C2 {
  public function __invoke(&$a0) {
    var_dump($a0);
    return $a0++;
  }
}
<<__EntryPoint>> function main(): void {
$x = 0;
$c = new C2;
$c(&$x);
var_dump($x);
 // $x = 1
}
