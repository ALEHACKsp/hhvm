<?hh
<<__EntryPoint>> function main(): void {
$strVals = varray[
   "0","65","-44", "1.2", "-7.7", "abc", "123abc", "123e5", "123e5xyz", " 123abc", "123 abc", "123abc ", "3.4a",
   "a5.9"
];

error_reporting(E_ERROR);

foreach ($strVals as $strVal) {
   foreach($strVals as $otherVal) {
	   echo "--- testing: '$strVal' << '$otherVal' ---\n";
      var_dump(bin2hex((string)($strVal<<$otherVal)));
   }
}


echo "===DONE===\n";
}
