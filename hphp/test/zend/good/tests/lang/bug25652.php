<?hh

function testfunc ($var) {
    echo "testfunc $var\n";
}

class foo {
    public $arr = array('testfunc');
    function bar () {
        $this->arr[0]('testvalue');
    }
}
<<__EntryPoint>> function main(): void {
$a = new foo ();
$a->bar ();
}
