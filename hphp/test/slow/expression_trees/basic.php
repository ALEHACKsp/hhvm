<?hh

<<file:__EnableUnstableFeatures('expression_trees')>>

<<__EntryPoint>>
function test(): void {
  $et = MyVisitor`1 + foo("bar")`;

  $visitor = new MyVisitor();

  $res = $et->construct($visitor);

  echo($res);
}

final class MyVisitor {
  public static function intLiteral(
    int $i,
  ): ExprTree<MyVisitor, mixed, int> {
    return new ExprTree(
      null,
      null,
      (MyVisitor $_) ==> (string)$i,
      () ==> { throw new Exception(); },
    );
  }

  public static function stringLiteral(
    string $s
  ): ExprTree<MyVisitor, mixed, string> {
    return new ExprTree(
      null,
      null,
      (MyVisitor $_) ==> "\"$s\"",
      () ==> { throw new Exception(); },
    );
  }

  public function plus(ExprPos $_, mixed $lhs, mixed $rhs): string {
    if ($lhs is string && $rhs is string) {
      return "$lhs + $rhs";
    }
    return "lhs + rhs";
  }

  public function call(ExprPos $_, string $name, vec<mixed> $args): string {
    $call = "$name(";
    foreach ($args as $arg) {
      if ($arg is string) {
        $call = $call . "$arg, ";
      } else {
        $call = $call . "arg, ";
      }
    }
    return $call . ")" ;
  }

  public function splice(
    ExprPos $_,
    ExprTree<MyVisitor, mixed, mixed> $et,
  ): mixed {
    return $et->construct($this);
  }
}

final class ExprTree<TVisitor, TResult, TInfer>{
  public function __construct(
    private ?ExprPos $pos,
    private ?string $filepath,
    private (function(TVisitor): TResult) $ast,
    private (function(): TInfer) $err,
  ) {}

  public function construct(TVisitor $v): TResult {
    return ($this->ast)($v);
  }
}

final class ExprPos {
  public function __construct(
    private int $begin_line,
    private int $begin_col,
    private int $end_line,
    private int $end_col,
  ) {}
}
