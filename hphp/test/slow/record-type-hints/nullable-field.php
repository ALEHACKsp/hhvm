<?hh
final record A {
  x: ?dict,
  y: int,
}
<<__EntryPoint>> function main(): void {
$a = A['x'=>null, 'y'=>1];
\var_dump($a['x']);
$b = A['x'=>dict['a'=>42], 'y'=>2];
\var_dump($b['x']);
$b['y'] = null;
\var_dump($b['y']);
}
