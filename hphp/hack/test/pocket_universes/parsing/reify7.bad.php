<?hh // strict
<<file:__EnableUnstableFeatures(
    'pocket_universes',
)>>

class C {
  enum E {
    case type T;
    :@A (type reify T = int);
  }
}
