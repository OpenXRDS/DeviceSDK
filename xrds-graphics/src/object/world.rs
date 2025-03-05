use super::XrdsScene;

#[derive(Default)]
pub struct XrdsWorld {
    scenes: std::collections::HashMap<String, XrdsScene>,
}

impl XrdsWorld {
    pub fn new() -> Self {
        Self {
            scenes: std::collections::HashMap::new(),
        }
    }

    pub fn scene(&self, name: &str) -> Option<&XrdsScene> {
        self.scenes.get(name)
    }

    pub fn scene_mut(&mut self, name: &str) -> Option<&mut XrdsScene> {
        self.scenes.get_mut(name)
    }

    pub fn scenes(&self) -> impl Iterator<Item = (&String, &XrdsScene)> {
        self.scenes.iter()
    }

    pub fn scenes_mut(&mut self) -> impl Iterator<Item = (&String, &mut XrdsScene)> {
        self.scenes.iter_mut()
    }

    pub fn add_scene(&mut self, name: String, scene: XrdsScene) {
        self.scenes.insert(name, scene);
    }
}

// Implement the IntoIterator trait for XrdsWorld to directly iterate over it
impl IntoIterator for XrdsWorld {
    type Item = (String, XrdsScene);
    type IntoIter = std::collections::hash_map::IntoIter<String, XrdsScene>;

    fn into_iter(self) -> Self::IntoIter {
        self.scenes.into_iter()
    }
}

// Implement the Iterator trait for a reference to XrdsWorld
impl<'a> XrdsWorld {
    pub fn iter(&'a self) -> XrdsWorldIter<'a> {
        XrdsWorldIter {
            inner: self.scenes.iter(),
        }
    }
}

pub struct XrdsWorldIter<'a> {
    inner: std::collections::hash_map::Iter<'a, String, XrdsScene>,
}

impl<'a> Iterator for XrdsWorldIter<'a> {
    type Item = (&'a String, &'a XrdsScene);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

// Implement the Iterator trait for a mutable reference to XrdsWorld
impl<'a> XrdsWorld {
    pub fn iter_mut(&'a mut self) -> XrdsWorldIterMut<'a> {
        XrdsWorldIterMut {
            inner: self.scenes.iter_mut(),
        }
    }
}

pub struct XrdsWorldIterMut<'a> {
    inner: std::collections::hash_map::IterMut<'a, String, XrdsScene>,
}

impl<'a> Iterator for XrdsWorldIterMut<'a> {
    type Item = (&'a String, &'a mut XrdsScene);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
