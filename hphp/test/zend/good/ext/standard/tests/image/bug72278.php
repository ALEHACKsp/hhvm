<?hh <<__EntryPoint>> function main(): void {
$filename =  __DIR__ . DIRECTORY_SEPARATOR . 'bug72278.jpg';
var_dump(getimagesize($filename));
}
