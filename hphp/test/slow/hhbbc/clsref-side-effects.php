<?hh
function __autoload($name) {
  throw new Exception($name);
}

<<__EntryPoint>>
function main_clsref_side_effects() {
try {
  echo AAA::$a; //zend_fetch_var_address_helper
} catch (Exception $e) {
  try {
    echo AAA::XXX; //ZEND_FETCH_CONSTANT
  } catch (Exception $e) {
    try {
      echo AAA::foo(); //ZEND_INIT_STATIC_METHOD_CALL
    } catch (Exception $e) {
      try  {
        unset(AAA::$a); // ZEND_UNSET_VAR
      } catch (Exception $e){
        try {
          isset(AAAA::$a); // ZEND_ISSET_ISEMPTY_VAR
        } catch (Exception $e) {
          try  {
            $a = array("AAA", "foo");
            $a(); //ZEND_INIT_FCALL_BY_NAME
          } catch (Exception $e) {
            echo "All of them!\n";
          }
        }
      }
    }
  }
}
echo 'okey';
}
