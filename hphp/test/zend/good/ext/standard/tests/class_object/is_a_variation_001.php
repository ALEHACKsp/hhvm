<?hh
/* Prototype  : proto bool is_a(object object, string class_name)
 * Description: Returns true if the object is of this class or has this class as one of its parents
 * Source code: Zend/zend_builtin_functions.c
 * Alias to functions:
 */
// Note: basic use cases in Zend/tests/is_a.phpt
<<__EntryPoint>> function main(): void {
echo "*** Testing is_a() : usage variations ***\n";
// Initialise function arguments not being substituted (if any)
$class_name = 'stdClass';

//get an unset variable
$unset_var = 10;
unset ($unset_var);

//array of values to iterate over
$values = varray[

      // int data
      0,
      1,
      12345,
      -2345,

      // float data
      10.5,
      -10.5,
      10.1234567e10,
      10.7654321E-10,
      .5,

      // array data
      array(),
      varray[0],
      varray[1],
      varray[1, 2],
      darray['color' => 'red', 'item' => 'pen'],

      // null data
      NULL,
      null,

      // boolean data
      true,
      false,
      TRUE,
      FALSE,

      // empty data
      "",
      '',

      // string data
      "string",
      'String',

      // undefined data
      $undefined_var,

      // unset data
      $unset_var,
];

// loop through each element of the array for object

foreach($values as $value) {
      echo @"\nArg value $value \n";
      var_dump( is_a($value, $class_name) );
};

echo "Done";
}
