<?hh
class test {
    public $p1 = 1;
    public $p2 = 2;
    public $p3;
    public function __clone() {
    }
};
<<__EntryPoint>> function main(): void {
$obj = new test;
$obj->p2 = 'A';
$obj->p3 = 'B';
$copy = clone $obj;
$copy->p3 = 'C';
echo "Object\n";
var_dump($obj);
echo "Clown\n";
var_dump($copy);
echo "Done\n";
}
