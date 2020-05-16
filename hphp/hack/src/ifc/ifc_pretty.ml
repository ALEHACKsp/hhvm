(* Copyright (c) 2020, Facebook, Inc.
   All rights reserved. *)
open Core_kernel
open Format
open Ifc_types
module Env = Ifc_env
module Logic = Ifc_logic
module Utils = Ifc_utils

let comma_sep fmt () = fprintf fmt ",@ "

let blank_sep fmt () = fprintf fmt "@ "

let rec list pp_sep pp fmt = function
  | [] -> ()
  | [x] -> fprintf fmt "%a" pp x
  | x :: xs -> fprintf fmt "%a%a%a" pp x pp_sep () (list pp_sep pp) xs

let show_policy = function
  | Pbot -> "Bot"
  | Ptop -> "Top"
  | Ppurpose p -> p
  | Pfree_var (v, _s) -> Printf.sprintf "v%d" v
  | Pbound_var n -> Printf.sprintf "<bound%d>" n

let policy fmt p = fprintf fmt "%s" (show_policy p)

let prop =
  let rec conjuncts = function
    | Cconj (Cflow (a, b), Cflow (c, d))
      when equal_policy a d && equal_policy b c ->
      [`q (a, b)]
    | Cflow (a, (Pbot as b))
    | Cflow ((Ptop as b), a) ->
      [`q (a, b)]
    | Cconj (cl, cr) -> conjuncts cl @ conjuncts cr
    | Ctrue -> []
    | c -> [`c c]
  in
  let bv =
    let rec f = function
      | [] -> ['a']
      | 'z' :: n -> 'a' :: f n
      | c :: n -> Char.(of_int_exn (1 + to_int c)) :: n
    in
    (fun i -> String.of_char_list (Utils.funpow i ~f ~init:[]))
  in
  let pp_policy b fmt = function
    | Pbound_var n -> fprintf fmt "%s" (bv (b - n))
    | p -> policy fmt p
  in
  let rec aux b fmt =
    let pol = pp_policy b in
    function
    | [] -> fprintf fmt "True"
    | [`q (p1, p2)] -> fprintf fmt "%a = %a" pol p1 pol p2
    | [`c (Cflow (p1, p2))] -> fprintf fmt "%a < %a" pol p1 pol p2
    | [`c (Cquant (q, n, c))] ->
      fprintf
        fmt
        "@[<hov2>%s @[<h>%a@].@ %a@]"
        (match q with
        | Qexists -> "exists"
        | Qforall -> "forall")
        (list blank_sep pp_print_string)
        (snd
           (Utils.funpow
              n
              ~f:(fun (i, l) -> (i + 1, l @ [bv i]))
              ~init:(b + 1, [])))
        (aux (b + n))
        (conjuncts c)
    | [`c (Ccond ((p, x), ct, ce))] ->
      fprintf fmt "@[<hov>if %a < %s@" pol p x;
      let cct = conjuncts ct in
      let cce = conjuncts ce in
      fprintf fmt "then %a@ else %a@]" (aux b) cct (aux b) cce
    | l ->
      let pp = list comma_sep (fun fmt c -> aux b fmt [c]) in
      fprintf fmt "[@[<hov>%a@]]" pp l
  in
  (fun fmt c -> aux 0 fmt (conjuncts c))

let rec ptype fmt ty =
  let list sep l =
    let pp_sep fmt () = fprintf fmt "%s@ " sep in
    fprintf fmt "(@[<hov2>%a@])" (list pp_sep ptype) l
  in
  match ty with
  | Tprim p -> fprintf fmt "<%a>" policy p
  | Ttuple tl -> list "," tl
  | Tunion tl -> list " |" tl
  | Tinter tl -> list " &" tl

let locals fmt env =
  let pp_lenv fmt { le_vars } = LMap.make_pp Local_id.pp ptype fmt le_vars in
  let pp_lenv_opt fmt = function
    | Some lenv -> pp_lenv fmt lenv
    | None -> fprintf fmt "<empty>"
  in
  pp_lenv_opt fmt (Env.get_lenv_opt env Typing_cont_key.Next)

let env fmt env =
  fprintf fmt "@[<v>";
  fprintf fmt "@[<hov2>Locals:@ %a@]" locals env;
  fprintf fmt "@,Return: %a" ptype env.e_ret;
  let p = Logic.prop_conjoin (List.rev env.e_acc) in
  fprintf fmt "@,Constraints:@,  @[<v>%a@]" prop p;
  fprintf fmt "@]"
