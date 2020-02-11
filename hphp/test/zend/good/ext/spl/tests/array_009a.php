<?hh

class MyRecursiveArrayIterator extends ArrayIterator implements RecursiveIterator
{
    function hasChildren()
    {
        return is_array($this->current());
    }

    function getChildren()
    {
        return new MyRecursiveArrayIterator($this->current());
    }
}
<<__EntryPoint>> function main(): void {
$array = array(1, 2 => array(21, 22 => varray[221, 222], 23 => varray[231]), 3);

$dir = new RecursiveIteratorIterator(new MyRecursiveArrayIterator($array), RecursiveIteratorIterator::LEAVES_ONLY);

foreach ($dir as $file) {
    print "$file\n";
}

echo "===DONE===\n";
}
