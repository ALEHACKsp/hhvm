<?hh <<__EntryPoint>> function main(): void {
error_reporting(E_ALL & ~E_USER_NOTICE & ~E_NOTICE);

$it = new APCIterator('user');
for($i = 0; $i < 41; $i++) {
  apc_store("key$i", "value$i");
}
$vals = array();
foreach($it as $key=>$value) {
  $vals[$key] = $value['key'];
}
ksort(&$vals);
var_dump($vals);

echo "===DONE===\n";
}
