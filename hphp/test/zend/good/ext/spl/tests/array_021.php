<?hh

class foo extends ArrayObject
{
    public function seek($key)
    {
        echo __METHOD__ . "($key)\n";
        throw new Exception("hi");
    }
}
<<__EntryPoint>> function main(): void {
$test = new foo(array(1,2,3));

try
{
    $test->seek('bar');
}
catch (Exception $e)
{
    echo "got exception\n";
}

echo "===DONE===\n";
}
