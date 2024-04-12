fn fun(input: Vec<usize>, inner: &impl Fn(usize) -> usize) -> Vec<usize> {
    #[cfg(not(feature = "changes_outer"))]
    let outer = |i: usize| std::iter::repeat(i).take(i).map(inner).sum();

    #[cfg(feature = "changes_outer")]
    let outer = |i: usize| i;

    input.into_iter().map(outer).collect::<Vec<usize>>()
}

fn fn_ptr_inner(i: usize) -> usize {
    #[cfg(not(feature = "changes_fn_ptr"))]
    return i * i;
    
    #[cfg(feature = "changes_fn_ptr")]
    return i + i;
}

fn get_inner() -> Box<dyn Fn(usize) -> usize>{
    #[cfg(not(feature = "changes_dyn"))]
    let inner = |i| i*i;
    #[cfg(feature = "changes_dyn")]
    let inner = |i| i+i;
    return Box::new(inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_closure() {
        #[cfg(not(feature = "changes_inner"))]
        let inner = |i| i * i;

        #[cfg(feature = "changes_inner")]
        let inner = |i| i + i;

        let input = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let output = fun(input.clone(), &inner);
        let expected = vec![1, 8, 27, 64, 125, 216, 343, 512, 729, 1000];
        assert_eq!(output, expected);
    }

    #[test]
    fn test_fn_ptr() {
        let input = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let output = fun(input.clone(), &fn_ptr_inner);
        let expected = vec![1, 8, 27, 64, 125, 216, 343, 512, 729, 1000];
        assert_eq!(output, expected);
    }

    #[test]
    fn test_dyn_fn() {
        let inner = get_inner();
        
        let input = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let output = fun(input.clone(),&inner.as_ref());
        let expected = vec![1, 8, 27, 64, 125, 216, 343, 512, 729, 1000];
        assert_eq!(output, expected);
    }
}
