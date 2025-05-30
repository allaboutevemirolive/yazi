use std::collections::{HashMap, HashSet};

use mlua::Function;
use parking_lot::RwLock;
use yazi_boot::BOOT;
use yazi_fs::FolderStage;
use yazi_shared::{Id, RoCell, url::Url};

use crate::{Client, ID, PEERS, body::{Body, BodyBulk, BodyCd, BodyDelete, BodyHi, BodyHover, BodyLoad, BodyMount, BodyMove, BodyMoveItem, BodyRename, BodyTab, BodyTrash, BodyYank}};

pub static LOCAL: RoCell<RwLock<HashMap<String, HashMap<String, Function>>>> = RoCell::new();

pub static REMOTE: RoCell<RwLock<HashMap<String, HashMap<String, Function>>>> = RoCell::new();

macro_rules! sub {
	($var:ident) => {
		|plugin: &str, kind: &str, f: Function| {
			let mut var = $var.write();
			let Some(map) = var.get_mut(kind) else {
				var.insert(kind.to_owned(), HashMap::from_iter([(plugin.to_owned(), f)]));
				return true;
			};

			if !map.contains_key(plugin) {
				map.insert(plugin.to_owned(), f);
				return true;
			}
			false
		}
	};
}

macro_rules! unsub {
	($var:ident) => {
		|plugin: &str, kind: &str| {
			let mut var = $var.write();
			let Some(map) = var.get_mut(kind) else { return false };

			if map.remove(plugin).is_none() {
				return false;
			}
			if map.is_empty() {
				var.remove(kind);
			}
			true
		}
	};
}

pub struct Pubsub;

impl Pubsub {
	pub fn sub(plugin: &str, kind: &str, f: Function) -> bool { sub!(LOCAL)(plugin, kind, f) }

	pub fn sub_remote(plugin: &str, kind: &str, f: Function) -> bool {
		sub!(REMOTE)(plugin, kind, f) && Self::pub_from_hi()
	}

	pub fn unsub(plugin: &str, kind: &str) -> bool { unsub!(LOCAL)(plugin, kind) }

	pub fn unsub_remote(plugin: &str, kind: &str) -> bool {
		unsub!(REMOTE)(plugin, kind) && Self::pub_from_hi()
	}

	pub fn pub_(body: Body<'static>) { body.with_receiver(*ID).emit(); }

	pub fn pub_to(receiver: Id, body: Body<'static>) {
		if receiver == *ID {
			return Self::pub_(body);
		}

		let kind = body.kind();
		if receiver == 0 && Self::any_remote_own(kind) {
			Client::push(body);
		} else if PEERS.read().get(&receiver).is_some_and(|c| c.able(kind)) {
			Client::push(body.with_receiver(receiver));
		}
	}

	pub fn pub_from_hi() -> bool {
		let abilities = REMOTE.read().keys().cloned().collect();
		let abilities = BOOT.remote_events.union(&abilities).map(|s| s.as_str()).collect();

		Client::push(BodyHi::borrowed(abilities));
		true
	}

	pub fn pub_from_tab(idx: Id) {
		if LOCAL.read().contains_key("tab") {
			Self::pub_(BodyTab::owned(idx));
		}
		if PEERS.read().values().any(|p| p.able("tab")) {
			Client::push(BodyTab::owned(idx));
		}
		if BOOT.local_events.contains("tab") {
			BodyTab::owned(idx).with_receiver(*ID).flush();
		}
	}

	pub fn pub_from_cd(tab: Id, url: &Url) {
		if LOCAL.read().contains_key("cd") {
			Self::pub_(BodyCd::dummy(tab));
		}
		if PEERS.read().values().any(|p| p.able("cd")) {
			Client::push(BodyCd::borrowed(tab, url));
		}
		if BOOT.local_events.contains("cd") {
			BodyCd::borrowed(tab, url).with_receiver(*ID).flush();
		}
	}

	pub fn pub_from_load(tab: Id, url: &Url, stage: FolderStage) {
		if LOCAL.read().contains_key("load") {
			Self::pub_(BodyLoad::dummy(tab, url, stage));
		}
		if PEERS.read().values().any(|p| p.able("load")) {
			Client::push(BodyLoad::borrowed(tab, url, stage));
		}
		if BOOT.local_events.contains("load") {
			BodyLoad::borrowed(tab, url, stage).with_receiver(*ID).flush();
		}
	}

	pub fn pub_from_hover(tab: Id, url: Option<&Url>) {
		if LOCAL.read().contains_key("hover") {
			Self::pub_(BodyHover::dummy(tab));
		}
		if PEERS.read().values().any(|p| p.able("hover")) {
			Client::push(BodyHover::borrowed(tab, url));
		}
		if BOOT.local_events.contains("hover") {
			BodyHover::borrowed(tab, url).with_receiver(*ID).flush();
		}
	}

	pub fn pub_from_rename(tab: Id, from: &Url, to: &Url) {
		if LOCAL.read().contains_key("rename") {
			Self::pub_(BodyRename::dummy(tab, from, to));
		}
		if PEERS.read().values().any(|p| p.able("rename")) {
			Client::push(BodyRename::borrowed(tab, from, to));
		}
		if BOOT.local_events.contains("rename") {
			BodyRename::borrowed(tab, from, to).with_receiver(*ID).flush();
		}
	}

	pub fn pub_from_bulk(changes: HashMap<&Url, &Url>) {
		if LOCAL.read().contains_key("bulk") {
			Self::pub_(BodyBulk::owned(&changes));
		}
		if PEERS.read().values().any(|p| p.able("bulk")) {
			Client::push(BodyBulk::borrowed(&changes));
		}
		if BOOT.local_events.contains("bulk") {
			BodyBulk::borrowed(&changes).with_receiver(*ID).flush();
		}
	}

	pub fn pub_from_yank(cut: bool, urls: &HashSet<Url>) {
		if LOCAL.read().contains_key("@yank") {
			Self::pub_(BodyYank::dummy());
		}
		if Self::any_remote_own("@yank") {
			Client::push(BodyYank::borrowed(cut, urls));
		}
		if BOOT.local_events.contains("@yank") {
			BodyYank::borrowed(cut, urls).with_receiver(*ID).flush();
		}
	}

	pub(super) fn pub_from_move(items: Vec<BodyMoveItem>) {
		if PEERS.read().values().any(|p| p.able("move")) {
			Client::push(BodyMove::borrowed(&items));
		}
		if BOOT.local_events.contains("move") {
			BodyMove::borrowed(&items).with_receiver(*ID).flush();
		}
		if LOCAL.read().contains_key("move") {
			Self::pub_(BodyMove::owned(items));
		}
	}

	pub(super) fn pub_from_trash(urls: Vec<Url>) {
		if PEERS.read().values().any(|p| p.able("trash")) {
			Client::push(BodyTrash::borrowed(&urls));
		}
		if BOOT.local_events.contains("trash") {
			BodyTrash::borrowed(&urls).with_receiver(*ID).flush();
		}
		if LOCAL.read().contains_key("trash") {
			Self::pub_(BodyTrash::owned(urls));
		}
	}

	pub(super) fn pub_from_delete(urls: Vec<Url>) {
		if PEERS.read().values().any(|p| p.able("delete")) {
			Client::push(BodyDelete::borrowed(&urls));
		}
		if BOOT.local_events.contains("delete") {
			BodyDelete::borrowed(&urls).with_receiver(*ID).flush();
		}
		if LOCAL.read().contains_key("delete") {
			Self::pub_(BodyDelete::owned(urls));
		}
	}

	pub fn pub_from_mount() {
		if LOCAL.read().contains_key("mount") {
			Self::pub_(BodyMount::owned());
		}
		if PEERS.read().values().any(|p| p.able("mount")) {
			Client::push(BodyMount::owned());
		}
		if BOOT.local_events.contains("mount") {
			BodyMount::owned().with_receiver(*ID).flush();
		}
	}

	#[inline]
	fn any_remote_own(kind: &str) -> bool {
		REMOTE.read().contains_key(kind)  // Owned abilities
			|| PEERS.read().values().any(|p| p.able(kind))  // Remote peers' abilities
			|| BOOT.remote_events.contains(kind) // Owned abilities from the command-line argument
	}
}
