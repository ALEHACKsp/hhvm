<?hh


class c {
}

function main($a) {
  return $a[c::BAR];
}
<<__EntryPoint>> function main_entry(): void {
var_dump(main(array('hello there' => 'success')));
}
