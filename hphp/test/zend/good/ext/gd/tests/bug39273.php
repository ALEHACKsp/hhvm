<?hh <<__EntryPoint>> function main(): void {
$small = imagecreatetruecolor(10, 10);
$c1 = imagecolorallocatealpha($small, 255,0,0,50);
imagecolortransparent($small, 0);
imagealphablending($small, false);
imagefilledrectangle($small, 0,0, 6,6, $c1);

$width = 300;
$height = 300;
$srcw = imagesx($small);
$srch = imagesy($small);

$img = imagecreatetruecolor($width, $height);

imagecolortransparent($img, 0);
imagealphablending($img, false);
imagecopyresized($img, $small, 0,0, 0,0, $width, $height, $srcw, $srch);
imagesavealpha($img, true);

$c = imagecolorat($img, 0,0);
printf("%X", $c);
}
