use rhai::*;
use rhai::plugin::*;
use num_rational::Rational32;

#[export_module]
mod fraction_module {

    pub type Fraction = Rational32;

    #[rhai_fn(name = "Fraction")]
    pub fn new_fraction(t: i32) -> Fraction {
        Fraction::from_integer(t)
    }
    #[rhai_fn(name = "Fraction")]
    pub fn new_fraction2(numer: i32, denom: i32) -> Dynamic {
        if denom == 0 {
            ().into()
        } else {
            Dynamic::from(Fraction::new(numer, denom))
        }
    }

    // rounding
    //============

    #[rhai_fn(pure)]
    pub fn floor(a: &mut Fraction) -> i32 {
        *a.floor().numer()
    }
    #[rhai_fn(pure)]
    pub fn ceil(a: &mut Fraction) -> i32 {
        *a.ceil().numer()
    }
    #[rhai_fn(pure)]
    pub fn round(a: &mut Fraction) -> i32 {
        *a.round().numer()
    }

    // comparison
    //============

    #[rhai_fn(name = "==")]
    pub fn eq_ff(a: Fraction, b: Fraction) -> bool {
        a == b
    }
    #[rhai_fn(name = "==")]
    pub fn eq_fi(a: Fraction, b: i32) -> bool {
        a == Fraction::from_integer(b)
    }
    #[rhai_fn(name = "==")]
    pub fn eq_if(a: i32, b: Fraction) -> bool {
        Fraction::from_integer(a) == b
    }

    #[rhai_fn(name = "!=")]
    pub fn neq_ff(a: Fraction, b: Fraction) -> bool {
        a != b
    }
    #[rhai_fn(name = "!=")]
    pub fn neq_fi(a: Fraction, b: i32) -> bool {
        a != Fraction::from_integer(b)
    }
    #[rhai_fn(name = "!=")]
    pub fn neq_if(a: i32, b: Fraction) -> bool {
        Fraction::from_integer(a) != b
    }

    #[rhai_fn(name = ">")]
    pub fn gt_ff(a: Fraction, b: Fraction) -> bool {
        a > b
    }
    #[rhai_fn(name = ">")]
    pub fn gt_fi(a: Fraction, b: i32) -> bool {
        a > Fraction::from_integer(b)
    }
    #[rhai_fn(name = ">")]
    pub fn gt_if(a: i32, b: Fraction) -> bool {
        Fraction::from_integer(a) > b
    }

    #[rhai_fn(name = "<")]
    pub fn lt_ff(a: Fraction, b: Fraction) -> bool {
        a < b
    }
    #[rhai_fn(name = "<")]
    pub fn lt_fi(a: Fraction, b: i32) -> bool {
        a < Fraction::from_integer(b)
    }
    #[rhai_fn(name = "<")]
    pub fn lt_if(a: i32, b: Fraction) -> bool {
        Fraction::from_integer(a) < b
    }

    #[rhai_fn(name = ">=")]
    pub fn ge_ff(a: Fraction, b: Fraction) -> bool {
        a >= b
    }
    #[rhai_fn(name = ">=")]
    pub fn ge_fi(a: Fraction, b: i32) -> bool {
        a >= Fraction::from_integer(b)
    }
    #[rhai_fn(name = ">=")]
    pub fn ge_if(a: i32, b: Fraction) -> bool {
        Fraction::from_integer(a) >= b
    }

    #[rhai_fn(name = "<=")]
    pub fn le_ff(a: Fraction, b: Fraction) -> bool {
        a <= b
    }
    #[rhai_fn(name = "<=")]
    pub fn le_fi(a: Fraction, b: i32) -> bool {
        a <= Fraction::from_integer(b)
    }
    #[rhai_fn(name = "<=")]
    pub fn le_if(a: i32, b: Fraction) -> bool {
        Fraction::from_integer(a) <= b
    }

    // addition and subtraction
    //==========================

    #[rhai_fn(name = "+")]
    pub fn add_ff(a: Fraction, b: Fraction) -> Fraction {
        a + b
    }
    #[rhai_fn(name = "+")]
    pub fn add_fi(a: Fraction, b: i32) -> Fraction {
        a + Fraction::from_integer(b)
    }
    #[rhai_fn(name = "+")]
    pub fn add_if(a: i32, b: Fraction) -> Fraction {
        Fraction::from_integer(a) + b
    }

    #[rhai_fn(name = "-")]
    pub fn sub_ff(a: Fraction, b: Fraction) -> Fraction {
        a - b
    }
    #[rhai_fn(name = "-")]
    pub fn sub_fi(a: Fraction, b: i32) -> Fraction {
        a - Fraction::from_integer(b)
    }
    #[rhai_fn(name = "-")]
    pub fn sub_if(a: i32, b: Fraction) -> Fraction {
        Fraction::from_integer(a) - b
    }

    #[rhai_fn(return_raw, name = "+=")]
    pub fn add_assign_df(a: &mut Dynamic, b: Fraction) -> Result<(), Box<EvalAltResult>> {
        match a.type_name().split("::").last().unwrap() {
            "i32" => {
                *a = Dynamic::from(add_if(a.clone().cast(), b));
            }
            "Fraction" | "Ratio<i32>" => {
                *a = Dynamic::from(add_ff(a.clone().cast(), b));
            }
            typename => return Err(format!("Attempted to add Fraction to a value of type '{typename}'").into())
        }
        Ok(())
    }
    #[rhai_fn(name = "+=")]
    pub fn add_assign_fi(a: &mut Fraction, b: i32) {
        *a += Rational32::from_integer(b)
    }

    #[rhai_fn(return_raw, name = "-=")]
    pub fn sub_assign_df(a: &mut Dynamic, b: Fraction) -> Result<(), Box<EvalAltResult>> {
        match a.type_name().split("::").last().unwrap() {
            "i32" => {
                *a = Dynamic::from(sub_if(a.clone().cast(), b));
            }
            "Fraction" => {
                *a = Dynamic::from(sub_ff(a.clone().cast(), b));
            }
            typename => return Err(format!("Attempted to subtract Fraction from a value of type '{typename}'").into())
        }
        Ok(())
    }
    #[rhai_fn(name = "-=")]
    pub fn sub_assign_fi(a: &mut Fraction, b: i32) {
        *a -= Rational32::from_integer(b)
    }

    // multiplication and division
    //=============================

    #[rhai_fn(name = "*")]
    pub fn mul_ff(a: Fraction, b: Fraction) -> Fraction {
        a * b
    }
    #[rhai_fn(name = "*")]
    pub fn mul_fi(a: Fraction, b: i32) -> Fraction {
        a * Fraction::from_integer(b)
    }
    #[rhai_fn(name = "*")]
    pub fn mul_if(a: i32, b: Fraction) -> Fraction {
        Fraction::from_integer(a) * b
    }

    #[rhai_fn(name = "/")]
    pub fn div_ff(a: Fraction, b: Fraction) -> Fraction {
        a / b
    }
    #[rhai_fn(name = "/")]
    pub fn div_fi(a: Fraction, b: i32) -> Fraction {
        a / Fraction::from_integer(b)
    }
    #[rhai_fn(name = "/")]
    pub fn div_if(a: i32, b: Fraction) -> Fraction {
        Fraction::from_integer(a) / b
    }

    #[rhai_fn(return_raw, name = "*=")]
    pub fn mul_assign_df(a: &mut Dynamic, b: Fraction) -> Result<(), Box<EvalAltResult>> {
        match a.type_name().split("::").last().unwrap() {
            "i32" => {
                *a = Dynamic::from(mul_if(a.clone().cast(), b));
            }
            "Fraction" => {
                *a = Dynamic::from(mul_ff(a.clone().cast(), b));
            }
            typename => return Err(format!("Attempted to multiply a value of type '{typename}' with a Fraction").into())
        }
        Ok(())
    }
    #[rhai_fn(name = "*=")]
    pub fn mul_assign_fi(a: &mut Fraction, b: i32) {
        *a *= Rational32::from_integer(b)
    }

    #[rhai_fn(return_raw, name = "/=")]
    pub fn div_assign_df(a: &mut Dynamic, b: Fraction) -> Result<(), Box<EvalAltResult>> {
        match a.type_name().split("::").last().unwrap() {
            "i32" => {
                *a = Dynamic::from(div_if(a.clone().cast(), b));
            }
            "Fraction" => {
                *a = Dynamic::from(div_ff(a.clone().cast(), b));
            }
            typename => return Err(format!("Attempted to divide a value of type '{typename}' by a Fraction").into())
        }
        Ok(())
    }
    #[rhai_fn(name = "/=")]
    pub fn div_assign_fi(a: &mut Fraction, b: i32) {
        *a /= Rational32::from_integer(b)
    }

    // powers
    //=======

    #[rhai_fn(name = "**")]
    pub fn pow_fi(a: Fraction, b: i32) -> Fraction {
        a.pow(b)
    }

    #[rhai_fn(name = "**=")]
    pub fn pow_assign_fi(a: &mut Fraction, b: i32) {
        *a = a.pow(b);
    }
}

def_package! {
    pub FractionPackage(module)
    {
        combine_with_exported_module!(module, "fraction_module", fraction_module);
    } |> |_engine| {
    }
}
