<?hh


<<__EntryPoint>>
function main_array_fill_keys_default() {
$keys = array(
  1234,
  'foo',
);

var_dump(array_fill_keys($keys, 'foo'));
}
