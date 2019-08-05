<?hh <<__EntryPoint>> function main(): void {
ini_set("intl.error_level", E_WARNING);
$tz = IntlTimeZone::createEnumeration('NL');
var_dump(get_class($tz));
$count = count(iterator_to_array($tz));
var_dump($count >= 1);

$tz->rewind();
var_dump(in_array('Europe/Amsterdam', iterator_to_array($tz)));
echo "==DONE==";
}
