<?hh <<__EntryPoint>> function main(): void {
var_dump(mcrypt_module_is_block_algorithm_mode(MCRYPT_MODE_CBC));
var_dump(mcrypt_module_is_block_algorithm_mode(MCRYPT_MODE_ECB));
var_dump(mcrypt_module_is_block_algorithm_mode(MCRYPT_MODE_STREAM));
var_dump(mcrypt_module_is_block_algorithm_mode(MCRYPT_MODE_OFB));
}
