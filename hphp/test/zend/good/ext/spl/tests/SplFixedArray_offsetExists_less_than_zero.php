<?hh <<__EntryPoint>> function main(): void {
$array = new SplFixedArray(5);
if($array->offsetExists(-10) === false) {
    echo 'PASS';
}
}
