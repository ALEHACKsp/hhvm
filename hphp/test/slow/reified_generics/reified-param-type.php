<?hh

set_error_handler(
  (int $errno,
  string $errstr,
  string $errfile,
  int $errline,
  array $errcontext
  ) ==> {
    echo "ERROR: ".$errstr." on line ".(string)$errline."\n";
    return true;
  }
);

function f<reify T>(@T $x) { echo "done\n"; }

class C<reify T> {}

f<int>(1);
f<num>(1);
f<int>(1.1);
f<num>(1.1);
f<int>(true);

f<bool>(true);
f<bool>(1);

f<C<shape('x' => int, 'y' => string)>>(new C<shape('x' => int, 'y' => string)>());
f<C<shape('x' => int, 'y' => string)>>(new C<shape('x' => int, 'y' => int)>());

f<C<(int, string)>>(new C<(int, string)>());
f<C<(int, string)>>(new C<(int, int)>());
