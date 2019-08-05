<?hh
/* Prototype  : bool interface_exists(string classname [, bool autoload])
 * Description: Checks if the class exists
 * Source code: Zend/zend_builtin_functions.c
 * Alias to functions:
 */

function __autoload($class_name) {
    require_once $class_name . '.inc';
}
<<__EntryPoint>> function main(): void {
echo "*** Testing interface_exists() : test autoload default value ***\n";


var_dump(interface_exists("AutoInterface"));

echo "\nDONE\n";
}
