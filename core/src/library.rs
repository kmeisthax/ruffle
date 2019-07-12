use crate::backend::audio::SoundHandle;
use crate::button::Button;
use crate::character::Character;
use crate::display_object::DisplayObject;
use crate::font::Font;
use crate::graphic::Graphic;
use crate::movie_clip::MovieClip;
use crate::prelude::*;
use std::collections::HashMap;
use swf::CharacterId;

pub struct Library<'a> {
    characters: HashMap<CharacterId, Character<'a>>,
    jpeg_tables: Option<Vec<u8>>,
}

impl<'a> Library<'a> {
    pub fn new() -> Self {
        Library {
            characters: HashMap::new(),
            jpeg_tables: None,
        }
    }

    pub fn register_character(&mut self, id: CharacterId, character: Character<'a>) {
        // TODO(Herschel): What is the behavior if id already exists?
        self.characters.insert(id, character);
    }

    pub fn contains_character(&self, id: CharacterId) -> bool {
        self.characters.contains_key(&id)
    }

    pub fn get_character(&self, id: CharacterId) -> Option<&Character<'a>> {
        self.characters.get(&id)
    }

    pub fn get_character_mut(&mut self, id: CharacterId) -> Option<&mut Character<'a>> {
        self.characters.get_mut(&id)
    }

    pub fn instantiate_display_object(
        &self,
        id: CharacterId,
    ) -> Result<Box<DisplayObject<'a>>, Box<std::error::Error>> {
        let obj: Box<DisplayObject<'a>> = match self.characters.get(&id) {
            Some(Character::Graphic(graphic)) => graphic.clone(),
            Some(Character::MorphShape(morph_shape)) => morph_shape.clone(),
            Some(Character::MovieClip(movie_clip)) => movie_clip.clone(),
            Some(Character::Button(button)) => button.clone(),
            Some(Character::Text(text)) => text.clone(),
            Some(_) => return Err("Not a DisplayObject".into()),
            None => return Err("Character id doesn't exist".into()),
        };
        Ok(obj)
    }

    pub fn get_font(&self, id: CharacterId) -> Option<&Font> {
        if let Some(&Character::Font(ref font)) = self.characters.get(&id) {
            Some(font)
        } else {
            None
        }
    }

    pub fn get_sound(&self, id: CharacterId) -> Option<SoundHandle> {
        if let Some(Character::Sound(sound)) = self.characters.get(&id) {
            Some(*sound)
        } else {
            None
        }
    }

    pub fn set_jpeg_tables(&mut self, data: Vec<u8>) {
        self.jpeg_tables = Some(data);
    }

    pub fn jpeg_tables(&self) -> Option<&[u8]> {
        self.jpeg_tables.as_ref().map(|data| &data[..])
    }
}
