pub trait Animal {
    fn sound(&self) -> &str;

    fn set_treat(&mut self, treat: Treat);

    fn walk(&self) {
        println!("walking...");
    }
}

pub enum Treat {
    Bone,
    Ball,
}

struct Lion {
    treat: Option<Treat>,
}

impl Animal for Lion {
    fn sound(&self) -> &str {
        "roar"
    }

    fn set_treat(&mut self, treat: Treat) {
        self.treat = Some(treat);
    }
}

struct Dog(Option<Treat>);

impl Animal for Dog {
    fn sound(&self) -> &str {
        "wuff"
    }

    fn set_treat(&mut self, treat: Treat) {
        if let None = self.0 {
            self.0 = Some(treat);
        }
    }
}

fn sound_generic<T>(t: &T) -> String
where
    T: Animal,
{
    t.sound().to_string()
}

fn sound_dyn<'a>(t: &'a dyn Animal) -> String {
    t.sound().to_string()
}

fn set_treat_generic<T: Animal>(t: &mut T, treat: Treat) {
    t.set_treat(treat);
}

fn set_treat_dyn<'a>(t: &'a mut dyn Animal, treat: Treat) {
    t.set_treat(treat);
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_direct() {
        let lion = Lion { treat: None };
        let dog = Dog(None);

        assert_eq!(lion.sound(), "roar");
        assert_eq!(dog.sound(), "wuff");
    }

    #[test]
    fn test_generic() {
        let lion = Lion { treat: None };
        let dog = Dog(None);

        assert_eq!(sound_generic(&lion), "roar");
        assert_eq!(sound_generic(&dog), "wuff");
    }

    #[test]
    fn test_dyn() {
        let boxed_lion = Box::new(Lion { treat: None });
        let boxed_dog = Box::new(Dog(None));

        assert_eq!(sound_dyn(boxed_lion.as_ref()), "roar");
        assert_eq!(sound_dyn(boxed_dog.as_ref()), "wuff");
    }

    #[test]
    fn test_mut_direct() {
        let mut lion = Lion { treat: None };
        let mut dog = Dog(None);

        lion.set_treat(Treat::Bone);
        dog.set_treat(Treat::Ball);
    }

    #[test]
    fn test_mut_generic() {
        let mut lion = Lion { treat: None };
        let mut dog = Dog(None);

        set_treat_generic(&mut lion, Treat::Bone);
        set_treat_generic(&mut dog, Treat::Ball);
    }

    #[test]
    fn test_mut_dyn() {
        let mut boxed_lion = Box::new(Lion { treat: None });
        let mut boxed_dog = Box::new(Dog(None));

        set_treat_dyn(boxed_lion.as_mut(), Treat::Bone);
        set_treat_dyn(boxed_dog.as_mut(), Treat::Ball);
    }
}
