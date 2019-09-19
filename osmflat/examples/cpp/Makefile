CXXFLAGS := -std=c++11

FLATDATA_INCLUDE_DIR ?= /usr/include
FLATDATA_LIBRARY_DIR ?= /usr/lib
FLATDATA_LIBRARY ?= flatdata

pub_names: pub_names.cpp osm_generated.h
	$(CXX) -O3 $(CXXFLAGS) -I${FLATDATA_INCLUDE_DIR} $< -o $@ -L${FLATDATA_LIBRARY_DIR} -l${FLATDATA_LIBRARY} -lboost_filesystem -lboost_system
