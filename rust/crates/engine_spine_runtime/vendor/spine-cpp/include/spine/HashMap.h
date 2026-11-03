/******************************************************************************
 * Spine Runtimes License Agreement
 * Last updated April 5, 2025. Replaces all prior versions.
 *
 * Copyright (c) 2013-2025, Esoteric Software LLC
 *
 * Integration of the Spine Runtimes into software or otherwise creating
 * derivative works of the Spine Runtimes is permitted under the terms and
 * conditions of Section 2 of the Spine Editor License Agreement:
 * http://esotericsoftware.com/spine-editor-license
 *
 * Otherwise, it is permitted to integrate the Spine Runtimes into software
 * or otherwise create derivative works of the Spine Runtimes (collectively,
 * "Products"), provided that each user of the Products must obtain their own
 * Spine Editor license and redistribution of the Products in any form must
 * include this license and copyright notice.
 *
 * THE SPINE RUNTIMES ARE PROVIDED BY ESOTERIC SOFTWARE LLC "AS IS" AND ANY
 * EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
 * WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * DISCLAIMED. IN NO EVENT SHALL ESOTERIC SOFTWARE LLC BE LIABLE FOR ANY
 * DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES,
 * BUSINESS INTERRUPTION, OR LOSS OF USE, DATA, OR PROFITS) HOWEVER CAUSED AND
 * ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
 * THE SPINE RUNTIMES, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 *****************************************************************************/

#ifndef Spine_HashMap_h
#define Spine_HashMap_h

#include <spine/Vector.h>
#include <spine/SpineObject.h>
#include <spine/Property.h>
#include <type_traits>
#include <unordered_map>
#include <memory_resource>

// Required for new with line number and file name in MSVC
#ifdef _MSC_VER
#pragma warning(disable:4291)

#pragma warning(disable:4251)

#endif

namespace spine {
	template<typename K, typename V>
	class SP_API HashMap : public SpineObject {
	private:
		class Entry;

	public:
		class SP_API Pair {
		public:
			explicit Pair(K &k, V &v) : key(k), value(v) {}

			K &key;
			V &value;
		};

		class SP_API Entries {
		public:
			friend class HashMap;

			explicit Entries(Entry *entry) : _entry(NULL), _hasChecked(false) {
				_start.next = entry;
				_entry = &_start;
			}

			Pair next() {
				assert(_entry);
				assert(_hasChecked);
				_entry = _entry->next;
				Pair pair(_entry->_key, _entry->_value);
				_hasChecked = false;
				return pair;
			}

			bool hasNext() {
				_hasChecked = true;
				return _entry->next;
			}

		private:
			bool _hasChecked;
			Entry _start;
			Entry *_entry;
		};

		HashMap() :
				_propertyIndex(&_propertyNodeStorage),
				_head(NULL),
				_size(0) {
		}

		~HashMap() {
			clear();
		}

		void clear() {
			if constexpr (std::is_same<K, PropertyId>::value) {
				// Destroy every logical node; only raw storage may be reused.
				_propertyIndex.clear();
			} else {
				for (Entry *entry = _head; entry != NULL;) {
					Entry *next = entry->next;
					delete entry;
					entry = next;
				}
			}
			_head = NULL;
			_size = 0;
		}

		size_t size() {
			return _size;
		}

		void put(const K &key, const V &value) {
			Entry *entry;
			bool exists;
			if constexpr (std::is_same<K, PropertyId>::value) {
				// One complete-key lookup for both insert and replacement.
				auto inserted = _propertyIndex.try_emplace(key);
				entry = &inserted.first->second;
				exists = !inserted.second;
			} else {
				entry = find(key);
				exists = entry != NULL;
			}
			if (exists) {
				entry->_key = key;
				entry->_value = value;
			} else {
				if constexpr (!std::is_same<K, PropertyId>::value) {
					entry = new(__FILE__, __LINE__) Entry();
				}
				entry->_key = key;
				entry->_value = value;

				Entry *oldHead = _head;

				if (oldHead) {
					_head = entry;
					oldHead->prev = entry;
					entry->next = oldHead;
				} else {
					_head = entry;
				}
				_size++;
			}
		}

		bool addAll(Vector <K> &keys, const V &value) {
			size_t oldSize = _size;
			for (size_t i = 0; i < keys.size(); i++) {
				put(keys[i], value);
			}
			return _size != oldSize;
		}

		bool containsKey(const K &key) {
			return find(key) != NULL;
		}

		bool remove(const K &key) {
			Entry *entry = find(key);
			if (!entry) return false;

			Entry *prev = entry->prev;
			Entry *next = entry->next;

			if (prev) prev->next = next;
			else _head = next;
			if (next) next->prev = entry->prev;

			if constexpr (std::is_same<K, PropertyId>::value) _propertyIndex.erase(key);
			else delete entry;
			_size--;

			return true;
		}

		V operator[](const K &key) {
			Entry *entry = find(key);
			if (entry) return entry->_value;
			else {
				assert(false);
				return 0;
			}
		}

		Entries getEntries() const {
			return Entries(_head);
		}

	private:
		Entry *find(const K &key) {
			// Engine same-version backport: PropertyId lookup is on every official
			// hold/timeline decision. Retain the original list for Entries ordering;
			// standard integer Eq resolves bucket collisions. Other key types keep
			// the upstream path (notably AnimationPair mix configuration).
			if constexpr (std::is_same<K, PropertyId>::value) {
				auto found = _propertyIndex.find(key);
				return found == _propertyIndex.end() ? NULL : &found->second;
			}
			for (Entry *entry = _head; entry != NULL; entry = entry->next) {
				if (entry->_key == key)
					return entry;
			}
			return NULL;
		}

		class SP_API Entry : public SpineObject {
		public:
			K _key;
			V _value;
			Entry *next;
			Entry *prev;

			Entry() : next(NULL), prev(NULL) {}
		};

		// Per-map, thread-confined raw node reuse. Logical Entry lifetimes still
		// end at remove/clear, and all storage ends with this HashMap. Large
		// bucket arrays use the upstream resource directly, not the small pool.
		std::pmr::unsynchronized_pool_resource _propertyNodeStorage{std::pmr::pool_options{4096, 128}};
		std::pmr::unordered_map<PropertyId, Entry> _propertyIndex;
		Entry *_head;
		size_t _size;
	};
}

#endif /* Spine_HashMap_h */
