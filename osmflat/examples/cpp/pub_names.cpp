#include "osm_generated.h"
#include <cstring>
#include <iostream>

bool comp(const char *left, const char *right) {
  while (*right == *left) {
    if (*right == 0) {
      return true;
    }
    left++;
    right++;
  }
  return false;
}

void print_pubs(const osm::Osm &archive, std::pair<uint64_t, uint64_t> range) {
  auto tags = archive.tags();
  auto tags_index = archive.tags_index();
  const char *strings = archive.stringtable().char_ptr();

  auto is_pub = [&](std::pair<uint64_t, uint64_t> range) {
    for (auto idx : tags_index.slice(range)) {
      auto tag = tags[idx.value];
      const char *key = strings + tag.key_idx;
      if (comp(key, "amenity")) {
        const char *value = strings + tag.value_idx;
        return comp(value, "pub");
      }
    }
    return false;
  };

  if (!is_pub(range)) {
    return;
  }

  auto get_name = [&](std::pair<uint64_t, uint64_t> range) {
    for (auto idx : tags_index.slice(range)) {
      auto tag = tags[idx.value];
      const char *key = strings + tag.key_idx;
      if (comp(key, "name")) {
        return strings + tag.value_idx;
      }
    }
    return "unknown pub name";
  };

  std::cout << get_name(range) << std::endl;

  for (auto idx : tags_index.slice(range)) {
    auto tag = tags[idx.value];
    const char *key = strings + tag.key_idx;
    if (std::strncmp(strings + tag.key_idx, "addr:", 5) == 0) {
      const char *value = strings + tag.value_idx;
      std::cout << "  " << key << ": " << value << std::endl;
    }
  }
}

int main(int argc, char const *argv[]) {
  if (argc != 2) {
    std::cerr << "USAGE: pub_name <osmflat-archive>" << std::endl;
    return 1;
  }

  auto storage = flatdata::FileResourceStorage::create(argv[1]);
  auto archive = osm::Osm::open(std::move(storage));

  for (auto node : archive.nodes()) {
    print_pubs(archive, node.tags);
  }

  for (auto way : archive.ways()) {
    print_pubs(archive, way.tags);
  }

  return 0;
}
