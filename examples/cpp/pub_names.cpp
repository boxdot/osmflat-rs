#include "osm_generated.h"
#include <cstring>
#include <iostream>

void print_pubs(const osm::Osm &archive, std::pair<uint64_t, uint64_t> range) {
  auto tags = archive.tags();
  auto tags_index = archive.tags_index();
  const char *strings = archive.stringtable().char_ptr();

  bool is_pub = false;
  for (uint32_t idx = range.first; idx < range.second; ++idx) {
    auto tag = tags[tags_index[idx].value];
    const char *key = &strings[tag.key_idx];
    const char *value = &strings[tag.value_idx];
    if (std::strncmp(key, "amenity\0", 8) == 0 && std::strncmp(value, "pub\0", 4) == 0) {
      is_pub = true;
      break;
    }
  }

  if (is_pub) {
    bool has_name = false;
    for (uint32_t idx = range.first; idx < range.second; ++idx) {
      auto tag = tags[tags_index[idx].value];
      const char *key = &strings[tag.key_idx];
      if (std::strncmp(key, "name\0", 5) == 0) {
          const char *value = &strings[tag.value_idx];
          std::cout << value << std::endl;
          has_name = true;
        }
    }
    if (!has_name) {
      std::cout << "unknown pub name" << std::endl;
    }

    for (uint32_t idx = range.first; idx < range.second; ++idx) {
      auto tag = tags[tags_index[idx].value];
      const char *key = &strings[tag.key_idx];
      if (std::strncmp(&strings[tag.key_idx], "addr:", 5) == 0) {
        const char *value = &strings[tag.value_idx];
        std::cout << "  " << key << ": " << value << std::endl;
      }
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
