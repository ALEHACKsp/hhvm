<?hh

function f($a1, &$a2) {
  foreach ($a1 as $k1 => $v1) {
    foreach ($a2 as $k2 => $v2) {
      $a2[$k2] += $v1;
      yield $a2[$k2];
    }
  }
}

<<__EntryPoint>>
function main_2152() {
$a1 = array(1, 2);
$a2 = array(1, 2);
foreach (f($a1, &$a2) as $v) {
 var_dump($v);
 }
var_dump($a2[0], $a2[1]);
}
