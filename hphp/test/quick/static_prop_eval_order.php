<?hh
function f() { echo '1 '; }
function g() { echo '2 '; }

HH\autoload_set_paths(
  dict[
    'class' => dict[
      'c' => 'static_prop_eval_order.inc',
    ],
  ],
  __DIR__.'/',
);

$cls = 'C';
$cls::$x[0][f()] = g();
echo "\n";
