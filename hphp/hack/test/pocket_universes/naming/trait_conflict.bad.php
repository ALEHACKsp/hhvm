<?hh

abstract class Base {
  enum E {
    case type T;
    case T val;
  }
}

trait myTrait0 {
  require extends Base;
  enum E {
    :@S(type T = string, val = "foo");
  }
}

trait myTrait1 {
  require extends Base;
  enum E {
    :@S(type T = string, val = "foo");
  }
}

class C extends Base {
  use myTrait0;
  use myTrait1;
}
