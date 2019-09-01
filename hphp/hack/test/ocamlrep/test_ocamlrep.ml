open Core_kernel

(* primitive tests *)
external get_a : unit -> char = "get_a"

external get_five : unit -> int = "get_five"

external get_true : unit -> bool = "get_true"

external get_false : unit -> bool = "get_false"

(* option tests *)
external get_none : unit -> int option = "get_none"

external get_some_five : unit -> int option = "get_some_five"

external get_some_none : unit -> int option option = "get_some_none"

external get_some_some_five : unit -> int option option = "get_some_some_five"

(* list tests *)
external get_empty_list : unit -> int list = "get_empty_list"

external get_five_list : unit -> int list = "get_five_list"

external get_one_two_three_list : unit -> int list = "get_one_two_three_list"

external get_float_list : unit -> float list = "get_float_list"

(* struct tests *)
type foo = {
  a: int;
  b: bool;
}

type bar = {
  c: foo;
  d: int option list option;
}

external get_foo : unit -> foo = "get_foo"

external get_bar : unit -> bar = "get_bar"

(* string tests *)
external get_empty_string : unit -> string = "get_empty_string"

external get_a_string : unit -> string = "get_a_string"

external get_ab_string : unit -> string = "get_ab_string"

external get_abcde_string : unit -> string = "get_abcde_string"

external get_abcdefg_string : unit -> string = "get_abcdefg_string"

external get_abcdefgh_string : unit -> string = "get_abcdefgh_string"

(* float tests *)
external get_zero_float : unit -> float = "get_zero_float"

external get_one_two_float : unit -> float = "get_one_two_float"

(* variant tests *)
type fruit =
  | Apple
  | Orange of int
  | Pear of { num: int }
  | Kiwi

external get_apple : unit -> fruit = "get_apple"

external get_orange : unit -> fruit = "get_orange"

external get_pear : unit -> fruit = "get_pear"

external get_kiwi : unit -> fruit = "get_kiwi"

let test_char () =
  let x = get_a () in
  assert (x = 'a')

let test_int () =
  let x = get_five () in
  assert (x = 5)

let test_true () =
  let x = get_true () in
  assert x

let test_false () =
  let x = get_false () in
  assert (not x)

let test_none () =
  let opt = get_none () in
  assert (Option.is_none opt)

let test_some () =
  let opt = get_some_five () in
  match opt with
  | None -> assert false
  | Some x -> assert (x = 5)

let test_some_none () =
  let opt = get_some_none () in
  match opt with
  | None -> assert false
  | Some x -> assert (Option.is_none x)

let test_some_some_five () =
  let opt = get_some_some_five () in
  match opt with
  | None -> assert false
  | Some x ->
    (match x with
    | None -> assert false
    | Some y -> assert (y = 5))

let test_empty_list () =
  let lst = get_empty_list () in
  assert (List.length lst = 0);
  match lst with
  | [] -> ()
  | _ -> assert false

let test_five_list () =
  let lst = get_five_list () in
  assert (List.length lst = 1);
  match lst with
  | [5] -> ()
  | _ -> assert false

let test_one_two_three_list () =
  match get_one_two_three_list () with
  | [1; 2; 3] -> ()
  | _ -> assert false

let test_float_list () =
  match get_float_list () with
  | [1.0; 2.0; 3.0] -> ()
  | _ -> assert false

let test_foo () =
  match get_foo () with
  | { a = 25; b = true } -> ()
  | _ -> assert false

let test_bar () =
  match get_bar () with
  | { c = { a = 42; b = false }; d = Some [Some 88; None; Some 66] } -> ()
  | _ -> assert false

let test_empty_string () =
  let s = get_empty_string () in
  assert (String.length s = 0);
  assert (s = "")

let test_a_string () =
  let s = get_a_string () in
  assert (String.length s = 1);
  assert (s = "a")

let test_ab_string () =
  let s = get_ab_string () in
  assert (String.length s = 2);
  assert (s = "ab")

let test_abcde_string () =
  let s = get_abcde_string () in
  assert (String.length s = 5);
  assert (s = "abcde")

let test_abcdefg_string () =
  let s = get_abcdefg_string () in
  assert (String.length s = 7);
  assert (s = "abcdefg")

let test_abcdefgh_string () =
  let s = get_abcdefgh_string () in
  assert (String.length s = 8);
  assert (s = "abcdefgh")

let float_compare f1 f2 =
  let abs_diff = Float.abs (f1 -. f2) in
  abs_diff < 0.0001

let test_zero_float () =
  let f = get_zero_float () in
  assert (float_compare f 0.)

let test_one_two_float () =
  let f = get_one_two_float () in
  assert (float_compare f 1.2)

let test_apple () = assert (get_apple () = Apple)

let test_kiwi () = assert (get_kiwi () = Kiwi)

let test_orange () =
  match get_orange () with
  | Orange 39 -> ()
  | _ -> assert false

let test_pear () =
  match get_pear () with
  | Pear { num = 76 } -> ()
  | _ -> assert false

let test_cases =
  [ test_char;
    test_int;
    test_true;
    test_false;
    test_none;
    test_some;
    test_some_none;
    test_some_some_five;
    test_empty_list;
    test_five_list;
    test_one_two_three_list;
    test_float_list;
    test_foo;
    test_bar;
    test_empty_string;
    test_a_string;
    test_ab_string;
    test_abcde_string;
    test_abcdefg_string;
    test_abcdefgh_string;
    test_zero_float;
    test_one_two_float;
    test_apple;
    test_kiwi;
    test_orange;
    test_pear ]

let main () = List.iter test_cases (fun test -> test ())

let () = main ()
