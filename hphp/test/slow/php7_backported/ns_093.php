<?hh

// should not throw syntax errors

use Foo\Bar     \{ A };

use Foo\Bar\    { B };

use Foo\Bar
\{
    C
};

use Foo\Bar\
{
    D
};

echo "\nDone\n";
