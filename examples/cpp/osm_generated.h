
#pragma once

#include <flatdata/flatdata.h>
#include <cstdint>
#include <iostream>
#include <iomanip>

namespace osm { 
enum : uint64_t
{
// Max 40 bits value used to indicate null references
    INVALID_IDX = 1099511627775
};
} // namespace osm

namespace osm { 
enum : uint64_t
{
// All coordinate were scaled by this to convert them to integers
    COORD_SCALE = 1000000000
};
} // namespace osm

namespace osm { 


template< template < typename, int, int, int > class Member >
union HeaderTemplate
{
    using BboxLeftType = Member< int64_t, 0, 40, 62 >;
    BboxLeftType bbox_left;
    using BboxRightType = Member< int64_t, 40, 40, 62 >;
    BboxRightType bbox_right;
    using BboxTopType = Member< int64_t, 80, 40, 62 >;
    BboxTopType bbox_top;
    using BboxBottomType = Member< int64_t, 120, 40, 62 >;
    BboxBottomType bbox_bottom;
    using RequiredFeatureFirstIdxType = Member< uint64_t, 160, 40, 62 >;
    RequiredFeatureFirstIdxType required_feature_first_idx;
    using RequiredFeaturesSizeType = Member< uint32_t, 200, 4, 62 >;
    RequiredFeaturesSizeType required_features_size;
    using OptionalFeatureFirstIdxType = Member< uint64_t, 204, 40, 62 >;
    OptionalFeatureFirstIdxType optional_feature_first_idx;
    using OptionalFeaturesSizeType = Member< uint32_t, 244, 4, 62 >;
    OptionalFeaturesSizeType optional_features_size;
    using WritingprogramIdxType = Member< uint64_t, 248, 40, 62 >;
    WritingprogramIdxType writingprogram_idx;
    using SourceIdxType = Member< uint64_t, 288, 40, 62 >;
    SourceIdxType source_idx;
    using OsmosisReplicationTimestampType = Member< int64_t, 328, 64, 62 >;
    OsmosisReplicationTimestampType osmosis_replication_timestamp;
    using OsmosisReplicationSequenceNumberType = Member< int64_t, 392, 64, 62 >;
    OsmosisReplicationSequenceNumberType osmosis_replication_sequence_number;
    using OsmosisReplicationBaseUrlIdxType = Member< uint64_t, 456, 40, 62 >;
    OsmosisReplicationBaseUrlIdxType osmosis_replication_base_url_idx;

    /// Stream type accepted by the class
    using StreamType = typename Member< uint32_t, 0, 0, 0 >::StreamType;
    /// Mutable structure type
    using MutatorType = HeaderTemplate< flatdata::Writer >;
    /// Immutable structure type
    using AccessorType = HeaderTemplate< flatdata::Reader >;

    HeaderTemplate( );
    explicit HeaderTemplate( StreamType data );

    /// Get raw data stream
    StreamType data( ) const;
    /// Get structure schema
    static std::string schema( );
    /// Get structure name
    static std::string name( );
    /// Get structure size in bytes
    static constexpr size_t size_in_bytes( );

    bool operator==( const HeaderTemplate& other ) const;
    bool operator!=( const HeaderTemplate& other ) const;
    bool operator<( const HeaderTemplate& other ) const;
    operator HeaderTemplate< flatdata::Reader >( ) const;
    explicit operator bool( ) const;

    std::string to_string( ) const;
    std::string describe( ) const;

    static constexpr bool IS_OVERLAPPING_WITH_NEXT = false;

    /**
    * Private data member, should not be directly used.
    * Cannot be made private.
    * Please refer to C++ Standard, Chapter 9.2, Paragraph 19.
    * This union has to be kept standard-layout, which different access control prevents.
    */
    Member< uint32_t, 0, 0, 0 > _data;
};


typedef HeaderTemplate< flatdata::Reader > Header;
typedef HeaderTemplate< flatdata::Writer > HeaderMutator;

} // namespace osm

namespace osm { 


template< template < typename, int, int, int > class Member >
union TagTemplate
{
    using KeyIdxType = Member< uint64_t, 0, 40, 10 >;
    KeyIdxType key_idx;
    using ValueIdxType = Member< uint64_t, 40, 40, 10 >;
    ValueIdxType value_idx;

    /// Stream type accepted by the class
    using StreamType = typename Member< uint32_t, 0, 0, 0 >::StreamType;
    /// Mutable structure type
    using MutatorType = TagTemplate< flatdata::Writer >;
    /// Immutable structure type
    using AccessorType = TagTemplate< flatdata::Reader >;

    TagTemplate( );
    explicit TagTemplate( StreamType data );

    /// Get raw data stream
    StreamType data( ) const;
    /// Get structure schema
    static std::string schema( );
    /// Get structure name
    static std::string name( );
    /// Get structure size in bytes
    static constexpr size_t size_in_bytes( );

    bool operator==( const TagTemplate& other ) const;
    bool operator!=( const TagTemplate& other ) const;
    bool operator<( const TagTemplate& other ) const;
    operator TagTemplate< flatdata::Reader >( ) const;
    explicit operator bool( ) const;

    std::string to_string( ) const;
    std::string describe( ) const;

    static constexpr bool IS_OVERLAPPING_WITH_NEXT = false;

    /**
    * Private data member, should not be directly used.
    * Cannot be made private.
    * Please refer to C++ Standard, Chapter 9.2, Paragraph 19.
    * This union has to be kept standard-layout, which different access control prevents.
    */
    Member< uint32_t, 0, 0, 0 > _data;
};


typedef TagTemplate< flatdata::Reader > Tag;
typedef TagTemplate< flatdata::Writer > TagMutator;

} // namespace osm

namespace osm { 


template< template < typename, int, int, int > class Member >
union NodeTemplate
{
    using IdType = Member< int64_t, 0, 40, 20 >;
    IdType id;
    using LatType = Member< int64_t, 40, 40, 20 >;
    LatType lat;
    using LonType = Member< int64_t, 80, 40, 20 >;
    LonType lon;
    using TagFirstIdxType = Member< uint64_t, 120, 40, 20 >;
    TagFirstIdxType tag_first_idx;
    using TagsType = Member< std::pair< uint64_t, uint64_t >, 120, 40, 20 >;
    TagsType tags;

    /// Stream type accepted by the class
    using StreamType = typename Member< uint32_t, 0, 0, 0 >::StreamType;
    /// Mutable structure type
    using MutatorType = NodeTemplate< flatdata::Writer >;
    /// Immutable structure type
    using AccessorType = NodeTemplate< flatdata::Reader >;

    NodeTemplate( );
    explicit NodeTemplate( StreamType data );

    /// Get raw data stream
    StreamType data( ) const;
    /// Get structure schema
    static std::string schema( );
    /// Get structure name
    static std::string name( );
    /// Get structure size in bytes
    static constexpr size_t size_in_bytes( );

    bool operator==( const NodeTemplate& other ) const;
    bool operator!=( const NodeTemplate& other ) const;
    bool operator<( const NodeTemplate& other ) const;
    operator NodeTemplate< flatdata::Reader >( ) const;
    explicit operator bool( ) const;

    std::string to_string( ) const;
    std::string describe( ) const;

    static constexpr bool IS_OVERLAPPING_WITH_NEXT = true;

    /**
    * Private data member, should not be directly used.
    * Cannot be made private.
    * Please refer to C++ Standard, Chapter 9.2, Paragraph 19.
    * This union has to be kept standard-layout, which different access control prevents.
    */
    Member< uint32_t, 0, 0, 0 > _data;
};


typedef NodeTemplate< flatdata::Reader > Node;
typedef NodeTemplate< flatdata::Writer > NodeMutator;

} // namespace osm

namespace osm { 

/**
 * A struct indexing a node.
 */
template< template < typename, int, int, int > class Member >
union NodeIndexTemplate
{
    using ValueType = Member< uint64_t, 0, 40, 5 >;
    ValueType value;

    /// Stream type accepted by the class
    using StreamType = typename Member< uint32_t, 0, 0, 0 >::StreamType;
    /// Mutable structure type
    using MutatorType = NodeIndexTemplate< flatdata::Writer >;
    /// Immutable structure type
    using AccessorType = NodeIndexTemplate< flatdata::Reader >;

    NodeIndexTemplate( );
    explicit NodeIndexTemplate( StreamType data );

    /// Get raw data stream
    StreamType data( ) const;
    /// Get structure schema
    static std::string schema( );
    /// Get structure name
    static std::string name( );
    /// Get structure size in bytes
    static constexpr size_t size_in_bytes( );

    bool operator==( const NodeIndexTemplate& other ) const;
    bool operator!=( const NodeIndexTemplate& other ) const;
    bool operator<( const NodeIndexTemplate& other ) const;
    operator NodeIndexTemplate< flatdata::Reader >( ) const;
    explicit operator bool( ) const;

    std::string to_string( ) const;
    std::string describe( ) const;

    static constexpr bool IS_OVERLAPPING_WITH_NEXT = false;

    /**
    * Private data member, should not be directly used.
    * Cannot be made private.
    * Please refer to C++ Standard, Chapter 9.2, Paragraph 19.
    * This union has to be kept standard-layout, which different access control prevents.
    */
    Member< uint32_t, 0, 0, 0 > _data;
};

/**
 * A struct indexing a node.
 */
typedef NodeIndexTemplate< flatdata::Reader > NodeIndex;
typedef NodeIndexTemplate< flatdata::Writer > NodeIndexMutator;

} // namespace osm

namespace osm { 


template< template < typename, int, int, int > class Member >
union WayTemplate
{
    using IdType = Member< int64_t, 0, 40, 15 >;
    IdType id;
    using TagFirstIdxType = Member< uint64_t, 40, 40, 15 >;
    TagFirstIdxType tag_first_idx;
    using TagsType = Member< std::pair< uint64_t, uint64_t >, 40, 40, 15 >;
    TagsType tags;
    using RefFirstIdxType = Member< uint64_t, 80, 40, 15 >;
    RefFirstIdxType ref_first_idx;
    using RefsType = Member< std::pair< uint64_t, uint64_t >, 80, 40, 15 >;
    RefsType refs;

    /// Stream type accepted by the class
    using StreamType = typename Member< uint32_t, 0, 0, 0 >::StreamType;
    /// Mutable structure type
    using MutatorType = WayTemplate< flatdata::Writer >;
    /// Immutable structure type
    using AccessorType = WayTemplate< flatdata::Reader >;

    WayTemplate( );
    explicit WayTemplate( StreamType data );

    /// Get raw data stream
    StreamType data( ) const;
    /// Get structure schema
    static std::string schema( );
    /// Get structure name
    static std::string name( );
    /// Get structure size in bytes
    static constexpr size_t size_in_bytes( );

    bool operator==( const WayTemplate& other ) const;
    bool operator!=( const WayTemplate& other ) const;
    bool operator<( const WayTemplate& other ) const;
    operator WayTemplate< flatdata::Reader >( ) const;
    explicit operator bool( ) const;

    std::string to_string( ) const;
    std::string describe( ) const;

    static constexpr bool IS_OVERLAPPING_WITH_NEXT = true;

    /**
    * Private data member, should not be directly used.
    * Cannot be made private.
    * Please refer to C++ Standard, Chapter 9.2, Paragraph 19.
    * This union has to be kept standard-layout, which different access control prevents.
    */
    Member< uint32_t, 0, 0, 0 > _data;
};


typedef WayTemplate< flatdata::Reader > Way;
typedef WayTemplate< flatdata::Writer > WayMutator;

} // namespace osm

namespace osm { 

/**
 * A struct indexing a tag.
 */
template< template < typename, int, int, int > class Member >
union TagIndexTemplate
{
    using ValueType = Member< uint64_t, 0, 40, 5 >;
    ValueType value;

    /// Stream type accepted by the class
    using StreamType = typename Member< uint32_t, 0, 0, 0 >::StreamType;
    /// Mutable structure type
    using MutatorType = TagIndexTemplate< flatdata::Writer >;
    /// Immutable structure type
    using AccessorType = TagIndexTemplate< flatdata::Reader >;

    TagIndexTemplate( );
    explicit TagIndexTemplate( StreamType data );

    /// Get raw data stream
    StreamType data( ) const;
    /// Get structure schema
    static std::string schema( );
    /// Get structure name
    static std::string name( );
    /// Get structure size in bytes
    static constexpr size_t size_in_bytes( );

    bool operator==( const TagIndexTemplate& other ) const;
    bool operator!=( const TagIndexTemplate& other ) const;
    bool operator<( const TagIndexTemplate& other ) const;
    operator TagIndexTemplate< flatdata::Reader >( ) const;
    explicit operator bool( ) const;

    std::string to_string( ) const;
    std::string describe( ) const;

    static constexpr bool IS_OVERLAPPING_WITH_NEXT = false;

    /**
    * Private data member, should not be directly used.
    * Cannot be made private.
    * Please refer to C++ Standard, Chapter 9.2, Paragraph 19.
    * This union has to be kept standard-layout, which different access control prevents.
    */
    Member< uint32_t, 0, 0, 0 > _data;
};

/**
 * A struct indexing a tag.
 */
typedef TagIndexTemplate< flatdata::Reader > TagIndex;
typedef TagIndexTemplate< flatdata::Writer > TagIndexMutator;

} // namespace osm

namespace osm { 


template< template < typename, int, int, int > class Member >
union NodeMemberTemplate
{
    using NodeIdxType = Member< uint64_t, 0, 40, 10 >;
    NodeIdxType node_idx;
    using RoleIdxType = Member< uint64_t, 40, 40, 10 >;
    RoleIdxType role_idx;

    /// Stream type accepted by the class
    using StreamType = typename Member< uint32_t, 0, 0, 0 >::StreamType;
    /// Mutable structure type
    using MutatorType = NodeMemberTemplate< flatdata::Writer >;
    /// Immutable structure type
    using AccessorType = NodeMemberTemplate< flatdata::Reader >;

    NodeMemberTemplate( );
    explicit NodeMemberTemplate( StreamType data );

    /// Get raw data stream
    StreamType data( ) const;
    /// Get structure schema
    static std::string schema( );
    /// Get structure name
    static std::string name( );
    /// Get structure size in bytes
    static constexpr size_t size_in_bytes( );

    bool operator==( const NodeMemberTemplate& other ) const;
    bool operator!=( const NodeMemberTemplate& other ) const;
    bool operator<( const NodeMemberTemplate& other ) const;
    operator NodeMemberTemplate< flatdata::Reader >( ) const;
    explicit operator bool( ) const;

    std::string to_string( ) const;
    std::string describe( ) const;

    static constexpr bool IS_OVERLAPPING_WITH_NEXT = false;

    /**
    * Private data member, should not be directly used.
    * Cannot be made private.
    * Please refer to C++ Standard, Chapter 9.2, Paragraph 19.
    * This union has to be kept standard-layout, which different access control prevents.
    */
    Member< uint32_t, 0, 0, 0 > _data;
};


typedef NodeMemberTemplate< flatdata::Reader > NodeMember;
typedef NodeMemberTemplate< flatdata::Writer > NodeMemberMutator;

} // namespace osm

namespace osm { 


template< template < typename, int, int, int > class Member >
union WayMemberTemplate
{
    using WayIdxType = Member< uint64_t, 0, 40, 10 >;
    WayIdxType way_idx;
    using RoleIdxType = Member< uint64_t, 40, 40, 10 >;
    RoleIdxType role_idx;

    /// Stream type accepted by the class
    using StreamType = typename Member< uint32_t, 0, 0, 0 >::StreamType;
    /// Mutable structure type
    using MutatorType = WayMemberTemplate< flatdata::Writer >;
    /// Immutable structure type
    using AccessorType = WayMemberTemplate< flatdata::Reader >;

    WayMemberTemplate( );
    explicit WayMemberTemplate( StreamType data );

    /// Get raw data stream
    StreamType data( ) const;
    /// Get structure schema
    static std::string schema( );
    /// Get structure name
    static std::string name( );
    /// Get structure size in bytes
    static constexpr size_t size_in_bytes( );

    bool operator==( const WayMemberTemplate& other ) const;
    bool operator!=( const WayMemberTemplate& other ) const;
    bool operator<( const WayMemberTemplate& other ) const;
    operator WayMemberTemplate< flatdata::Reader >( ) const;
    explicit operator bool( ) const;

    std::string to_string( ) const;
    std::string describe( ) const;

    static constexpr bool IS_OVERLAPPING_WITH_NEXT = false;

    /**
    * Private data member, should not be directly used.
    * Cannot be made private.
    * Please refer to C++ Standard, Chapter 9.2, Paragraph 19.
    * This union has to be kept standard-layout, which different access control prevents.
    */
    Member< uint32_t, 0, 0, 0 > _data;
};


typedef WayMemberTemplate< flatdata::Reader > WayMember;
typedef WayMemberTemplate< flatdata::Writer > WayMemberMutator;

} // namespace osm

namespace osm { 


template< template < typename, int, int, int > class Member >
union RelationMemberTemplate
{
    using RelationIdxType = Member< uint64_t, 0, 40, 10 >;
    RelationIdxType relation_idx;
    using RoleIdxType = Member< uint64_t, 40, 40, 10 >;
    RoleIdxType role_idx;

    /// Stream type accepted by the class
    using StreamType = typename Member< uint32_t, 0, 0, 0 >::StreamType;
    /// Mutable structure type
    using MutatorType = RelationMemberTemplate< flatdata::Writer >;
    /// Immutable structure type
    using AccessorType = RelationMemberTemplate< flatdata::Reader >;

    RelationMemberTemplate( );
    explicit RelationMemberTemplate( StreamType data );

    /// Get raw data stream
    StreamType data( ) const;
    /// Get structure schema
    static std::string schema( );
    /// Get structure name
    static std::string name( );
    /// Get structure size in bytes
    static constexpr size_t size_in_bytes( );

    bool operator==( const RelationMemberTemplate& other ) const;
    bool operator!=( const RelationMemberTemplate& other ) const;
    bool operator<( const RelationMemberTemplate& other ) const;
    operator RelationMemberTemplate< flatdata::Reader >( ) const;
    explicit operator bool( ) const;

    std::string to_string( ) const;
    std::string describe( ) const;

    static constexpr bool IS_OVERLAPPING_WITH_NEXT = false;

    /**
    * Private data member, should not be directly used.
    * Cannot be made private.
    * Please refer to C++ Standard, Chapter 9.2, Paragraph 19.
    * This union has to be kept standard-layout, which different access control prevents.
    */
    Member< uint32_t, 0, 0, 0 > _data;
};


typedef RelationMemberTemplate< flatdata::Reader > RelationMember;
typedef RelationMemberTemplate< flatdata::Writer > RelationMemberMutator;

} // namespace osm

namespace osm { 


template< template < typename, int, int, int > class Member >
union RelationTemplate
{
    using IdType = Member< int64_t, 0, 40, 10 >;
    IdType id;
    using TagFirstIdxType = Member< uint64_t, 40, 40, 10 >;
    TagFirstIdxType tag_first_idx;
    using TagsType = Member< std::pair< uint64_t, uint64_t >, 40, 40, 10 >;
    TagsType tags;

    /// Stream type accepted by the class
    using StreamType = typename Member< uint32_t, 0, 0, 0 >::StreamType;
    /// Mutable structure type
    using MutatorType = RelationTemplate< flatdata::Writer >;
    /// Immutable structure type
    using AccessorType = RelationTemplate< flatdata::Reader >;

    RelationTemplate( );
    explicit RelationTemplate( StreamType data );

    /// Get raw data stream
    StreamType data( ) const;
    /// Get structure schema
    static std::string schema( );
    /// Get structure name
    static std::string name( );
    /// Get structure size in bytes
    static constexpr size_t size_in_bytes( );

    bool operator==( const RelationTemplate& other ) const;
    bool operator!=( const RelationTemplate& other ) const;
    bool operator<( const RelationTemplate& other ) const;
    operator RelationTemplate< flatdata::Reader >( ) const;
    explicit operator bool( ) const;

    std::string to_string( ) const;
    std::string describe( ) const;

    static constexpr bool IS_OVERLAPPING_WITH_NEXT = true;

    /**
    * Private data member, should not be directly used.
    * Cannot be made private.
    * Please refer to C++ Standard, Chapter 9.2, Paragraph 19.
    * This union has to be kept standard-layout, which different access control prevents.
    */
    Member< uint32_t, 0, 0, 0 > _data;
};


typedef RelationTemplate< flatdata::Reader > Relation;
typedef RelationTemplate< flatdata::Writer > RelationMutator;

} // namespace osm

namespace _builtin { namespace multivector { 

/** Builtin type to for MultiVector index */
template< template < typename, int, int, int > class Member >
union IndexType40Template
{
    using ValueType = Member< uint64_t, 0, 40, 5 >;
    ValueType value;
    using RangeType = Member< std::pair< uint64_t, uint64_t >, 0, 40, 5 >;
    RangeType range;

    /// Stream type accepted by the class
    using StreamType = typename Member< uint32_t, 0, 0, 0 >::StreamType;
    /// Mutable structure type
    using MutatorType = IndexType40Template< flatdata::Writer >;
    /// Immutable structure type
    using AccessorType = IndexType40Template< flatdata::Reader >;

    IndexType40Template( );
    explicit IndexType40Template( StreamType data );

    /// Get raw data stream
    StreamType data( ) const;
    /// Get structure schema
    static std::string schema( );
    /// Get structure name
    static std::string name( );
    /// Get structure size in bytes
    static constexpr size_t size_in_bytes( );

    bool operator==( const IndexType40Template& other ) const;
    bool operator!=( const IndexType40Template& other ) const;
    bool operator<( const IndexType40Template& other ) const;
    operator IndexType40Template< flatdata::Reader >( ) const;
    explicit operator bool( ) const;

    std::string to_string( ) const;
    std::string describe( ) const;

    static constexpr bool IS_OVERLAPPING_WITH_NEXT = true;

    /**
    * Private data member, should not be directly used.
    * Cannot be made private.
    * Please refer to C++ Standard, Chapter 9.2, Paragraph 19.
    * This union has to be kept standard-layout, which different access control prevents.
    */
    Member< uint32_t, 0, 0, 0 > _data;
};

/** Builtin type to for MultiVector index */
typedef IndexType40Template< flatdata::Reader > IndexType40;
typedef IndexType40Template< flatdata::Writer > IndexType40Mutator;

}} // namespace _builtin.multivector

namespace osm { 

class Osm : public flatdata::Archive
{
public:
    /// Archive schema
    static const char* schema_definition( );
    /// Archive name
    static const char* name_definition( );

public:
    /**
    * Create and open archive at path.
    * In case opening fails, is_open() or operator bool() returns false.
    *
    * @sa is_open
    * @sa operator bool()
    */
    static Osm open( std::shared_ptr< flatdata::ResourceStorage > storage );
    Osm( ) = default;

    using HeaderType = ::osm::Header;
    const HeaderType& header( ) const;

    using NodesType = flatdata::ArrayView< ::osm::Node >;
    const NodesType& nodes( ) const;

    using WaysType = flatdata::ArrayView< ::osm::Way >;
    const WaysType& ways( ) const;

    using RelationsType = flatdata::ArrayView< ::osm::Relation >;
    const RelationsType& relations( ) const;

    using RelationMembersType = flatdata::MultiArrayView< ::_builtin::multivector::IndexType40, ::osm::NodeMember, ::osm::WayMember, ::osm::RelationMember >;
    const RelationMembersType& relation_members( ) const;

    using TagsType = flatdata::ArrayView< ::osm::Tag >;
    const TagsType& tags( ) const;

    using TagsIndexType = flatdata::ArrayView< ::osm::TagIndex >;
    const TagsIndexType& tags_index( ) const;

    using NodesIndexType = flatdata::ArrayView< ::osm::NodeIndex >;
    const NodesIndexType& nodes_index( ) const;


    /**
     * List of strings separated by \0.
     */
    using StringtableType = flatdata::MemoryDescriptor;
    const StringtableType& stringtable( ) const;


    const char* name( ) const override;
    const char* schema( ) const override;

private:
    explicit Osm( std::shared_ptr< flatdata::ResourceStorage > storage );

    bool load_contents( ) override;
    void describe_resources( std::ostream& stream ) const override;

private:
    HeaderType m_header;
    NodesType m_nodes;
    WaysType m_ways;
    RelationsType m_relations;
    RelationMembersType m_relation_members;
    TagsType m_tags;
    TagsIndexType m_tags_index;
    NodesIndexType m_nodes_index;
    StringtableType m_stringtable;
};

class OsmBuilder : public flatdata::ArchiveBuilder
{
public:
    /// Creates Archive builder
    static OsmBuilder open( std::shared_ptr< flatdata::ResourceStorage > storage );
    /// Archive schema
    static const char* schema_definition( );

public:  /// Common methods
    OsmBuilder( ) = default;
    const char* name( ) const override;
    const char* schema( ) const override;

public:  /// Resources
    using HeaderType = ::osm::Header;
    using HeaderReaderType = ::osm::Header;
    bool set_header( HeaderReaderType data );

    using NodesType = flatdata::ExternalVector< ::osm::Node >;
    using NodesReaderType = flatdata::ArrayView< ::osm::Node >;
    NodesType start_nodes( );
    bool set_nodes( NodesReaderType data );

    using WaysType = flatdata::ExternalVector< ::osm::Way >;
    using WaysReaderType = flatdata::ArrayView< ::osm::Way >;
    WaysType start_ways( );
    bool set_ways( WaysReaderType data );

    using RelationsType = flatdata::ExternalVector< ::osm::Relation >;
    using RelationsReaderType = flatdata::ArrayView< ::osm::Relation >;
    RelationsType start_relations( );
    bool set_relations( RelationsReaderType data );

    using RelationMembersType = flatdata::MultiVector< ::_builtin::multivector::IndexType40, ::osm::NodeMember, ::osm::WayMember, ::osm::RelationMember >;
    using RelationMembersReaderType = flatdata::MultiArrayView< ::_builtin::multivector::IndexType40, ::osm::NodeMember, ::osm::WayMember, ::osm::RelationMember >;
    RelationMembersType start_relation_members( );

    using TagsType = flatdata::ExternalVector< ::osm::Tag >;
    using TagsReaderType = flatdata::ArrayView< ::osm::Tag >;
    TagsType start_tags( );
    bool set_tags( TagsReaderType data );

    using TagsIndexType = flatdata::ExternalVector< ::osm::TagIndex >;
    using TagsIndexReaderType = flatdata::ArrayView< ::osm::TagIndex >;
    TagsIndexType start_tags_index( );
    bool set_tags_index( TagsIndexReaderType data );

    using NodesIndexType = flatdata::ExternalVector< ::osm::NodeIndex >;
    using NodesIndexReaderType = flatdata::ArrayView< ::osm::NodeIndex >;
    NodesIndexType start_nodes_index( );
    bool set_nodes_index( NodesIndexReaderType data );

    using StringtableType = flatdata::MemoryDescriptor;
    using StringtableReaderType = flatdata::MemoryDescriptor;
    bool set_stringtable( StringtableReaderType data );



private:
    OsmBuilder( std::shared_ptr< flatdata::ResourceStorage > storage );

};

} // namespace osm


// -------------------------------------------------------------------------------------------------
// -------------------------------------- Implementations ------------------------------------------
// -------------------------------------------------------------------------------------------------

namespace osm { 
namespace internal
{
    const char* const Header__schema__ = R"schema(namespace osm {
struct Header
{
    bbox_left : i64 : 40;
    bbox_right : i64 : 40;
    bbox_top : i64 : 40;
    bbox_bottom : i64 : 40;
    required_feature_first_idx : u64 : 40;
    required_features_size : u32 : 4;
    optional_feature_first_idx : u64 : 40;
    optional_features_size : u32 : 4;
    writingprogram_idx : u64 : 40;
    source_idx : u64 : 40;
    osmosis_replication_timestamp : i64 : 64;
    osmosis_replication_sequence_number : i64 : 64;
    osmosis_replication_base_url_idx : u64 : 40;
}
}

)schema";
}

template< template < typename, int, int, int > class Member >
inline
HeaderTemplate< Member >::HeaderTemplate( )
: _data( Member< uint32_t, 0, 0, 0 >{nullptr} )
{
}

template< template < typename, int, int, int > class Member >
inline
HeaderTemplate< Member >::HeaderTemplate( StreamType data )
: _data( Member< uint32_t, 0, 0, 0 >{data} )
{
}

template< template < typename, int, int, int > class Member >
inline
HeaderTemplate< Member >::operator bool( ) const
{
return _data.data != nullptr;
}

template< template < typename, int, int, int > class Member >
inline
typename HeaderTemplate< Member >::StreamType HeaderTemplate< Member >::data( ) const { return _data.data; }

template< template < typename, int, int, int > class Member >
inline
std::string HeaderTemplate< Member >::schema( ) { return internal::Header__schema__; }

template< template < typename, int, int, int > class Member >
inline
std::string HeaderTemplate< Member >::name( ) { return "Header"; }

template< template < typename, int, int, int > class Member >
inline
constexpr size_t HeaderTemplate< Member >::size_in_bytes( ) { return 62; }

template< template < typename, int, int, int > class Member >
inline
bool HeaderTemplate< Member >::operator==( const HeaderTemplate& other ) const
{
    for ( size_t i = 0; i < size_in_bytes( ); i++ )
    {
        if ( _data.data[ i ] != other._data.data[ i ] )
        {
            return false;
        }
    }
    return true;
}

template< template < typename, int, int, int > class Member >
inline
bool HeaderTemplate< Member >::operator!=( const HeaderTemplate& other ) const
{
    return !( *this == other );
}

template< template < typename, int, int, int > class Member >
inline
bool HeaderTemplate< Member >::operator<( const HeaderTemplate& other ) const
{
return
    bbox_left < other.bbox_left &&
    bbox_right < other.bbox_right &&
    bbox_top < other.bbox_top &&
    bbox_bottom < other.bbox_bottom &&
    required_feature_first_idx < other.required_feature_first_idx &&
    required_features_size < other.required_features_size &&
    optional_feature_first_idx < other.optional_feature_first_idx &&
    optional_features_size < other.optional_features_size &&
    writingprogram_idx < other.writingprogram_idx &&
    source_idx < other.source_idx &&
    osmosis_replication_timestamp < other.osmosis_replication_timestamp &&
    osmosis_replication_sequence_number < other.osmosis_replication_sequence_number &&
    osmosis_replication_base_url_idx < other.osmosis_replication_base_url_idx ;
}

template< template < typename, int, int, int > class Member >
inline
HeaderTemplate< Member >::operator HeaderTemplate< flatdata::Reader >( ) const
{
    return HeaderTemplate< flatdata::Reader >( _data.data );
}

template< template < typename, int, int, int > class Member >
inline
std::string HeaderTemplate< Member >::to_string( ) const
{
    std::ostringstream ss;
    ss << "{ " << std::endl <<
    "bbox_left : " << static_cast< uint64_t >( bbox_left ) << ", " << std::endl
    <<
    "bbox_right : " << static_cast< uint64_t >( bbox_right ) << ", " << std::endl
    <<
    "bbox_top : " << static_cast< uint64_t >( bbox_top ) << ", " << std::endl
    <<
    "bbox_bottom : " << static_cast< uint64_t >( bbox_bottom ) << ", " << std::endl
    <<
    "required_feature_first_idx : " << static_cast< uint64_t >( required_feature_first_idx ) << ", " << std::endl
    <<
    "required_features_size : " << static_cast< uint64_t >( required_features_size ) << ", " << std::endl
    <<
    "optional_feature_first_idx : " << static_cast< uint64_t >( optional_feature_first_idx ) << ", " << std::endl
    <<
    "optional_features_size : " << static_cast< uint64_t >( optional_features_size ) << ", " << std::endl
    <<
    "writingprogram_idx : " << static_cast< uint64_t >( writingprogram_idx ) << ", " << std::endl
    <<
    "source_idx : " << static_cast< uint64_t >( source_idx ) << ", " << std::endl
    <<
    "osmosis_replication_timestamp : " << static_cast< uint64_t >( osmosis_replication_timestamp ) << ", " << std::endl
    <<
    "osmosis_replication_sequence_number : " << static_cast< uint64_t >( osmosis_replication_sequence_number ) << ", " << std::endl
    <<
    "osmosis_replication_base_url_idx : " << static_cast< uint64_t >( osmosis_replication_base_url_idx ) << ", " << std::endl
    << "}"
;
    return ss.str( );
}

template< template < typename, int, int, int > class Member >
inline
std::string HeaderTemplate< Member >::describe( ) const
{
    std::ostringstream ss;
    ss << "Structure of size " << size_in_bytes( );
    return ss.str( );
}
} // namespace osm

namespace osm { 
namespace internal
{
    const char* const Tag__schema__ = R"schema(namespace osm {
struct Tag
{
    key_idx : u64 : 40;
    value_idx : u64 : 40;
}
}

)schema";
}

template< template < typename, int, int, int > class Member >
inline
TagTemplate< Member >::TagTemplate( )
: _data( Member< uint32_t, 0, 0, 0 >{nullptr} )
{
}

template< template < typename, int, int, int > class Member >
inline
TagTemplate< Member >::TagTemplate( StreamType data )
: _data( Member< uint32_t, 0, 0, 0 >{data} )
{
}

template< template < typename, int, int, int > class Member >
inline
TagTemplate< Member >::operator bool( ) const
{
return _data.data != nullptr;
}

template< template < typename, int, int, int > class Member >
inline
typename TagTemplate< Member >::StreamType TagTemplate< Member >::data( ) const { return _data.data; }

template< template < typename, int, int, int > class Member >
inline
std::string TagTemplate< Member >::schema( ) { return internal::Tag__schema__; }

template< template < typename, int, int, int > class Member >
inline
std::string TagTemplate< Member >::name( ) { return "Tag"; }

template< template < typename, int, int, int > class Member >
inline
constexpr size_t TagTemplate< Member >::size_in_bytes( ) { return 10; }

template< template < typename, int, int, int > class Member >
inline
bool TagTemplate< Member >::operator==( const TagTemplate& other ) const
{
    for ( size_t i = 0; i < size_in_bytes( ); i++ )
    {
        if ( _data.data[ i ] != other._data.data[ i ] )
        {
            return false;
        }
    }
    return true;
}

template< template < typename, int, int, int > class Member >
inline
bool TagTemplate< Member >::operator!=( const TagTemplate& other ) const
{
    return !( *this == other );
}

template< template < typename, int, int, int > class Member >
inline
bool TagTemplate< Member >::operator<( const TagTemplate& other ) const
{
return
    key_idx < other.key_idx &&
    value_idx < other.value_idx ;
}

template< template < typename, int, int, int > class Member >
inline
TagTemplate< Member >::operator TagTemplate< flatdata::Reader >( ) const
{
    return TagTemplate< flatdata::Reader >( _data.data );
}

template< template < typename, int, int, int > class Member >
inline
std::string TagTemplate< Member >::to_string( ) const
{
    std::ostringstream ss;
    ss << "{ " << std::endl <<
    "key_idx : " << static_cast< uint64_t >( key_idx ) << ", " << std::endl
    <<
    "value_idx : " << static_cast< uint64_t >( value_idx ) << ", " << std::endl
    << "}"
;
    return ss.str( );
}

template< template < typename, int, int, int > class Member >
inline
std::string TagTemplate< Member >::describe( ) const
{
    std::ostringstream ss;
    ss << "Structure of size " << size_in_bytes( );
    return ss.str( );
}
} // namespace osm

namespace osm { 
namespace internal
{
    const char* const Node__schema__ = R"schema(namespace osm {
struct Node
{
    id : i64 : 40;
    lat : i64 : 40;
    lon : i64 : 40;
    @range( tags )
    tag_first_idx : u64 : 40;
}
}

)schema";
}

template< template < typename, int, int, int > class Member >
inline
NodeTemplate< Member >::NodeTemplate( )
: _data( Member< uint32_t, 0, 0, 0 >{nullptr} )
{
}

template< template < typename, int, int, int > class Member >
inline
NodeTemplate< Member >::NodeTemplate( StreamType data )
: _data( Member< uint32_t, 0, 0, 0 >{data} )
{
}

template< template < typename, int, int, int > class Member >
inline
NodeTemplate< Member >::operator bool( ) const
{
return _data.data != nullptr;
}

template< template < typename, int, int, int > class Member >
inline
typename NodeTemplate< Member >::StreamType NodeTemplate< Member >::data( ) const { return _data.data; }

template< template < typename, int, int, int > class Member >
inline
std::string NodeTemplate< Member >::schema( ) { return internal::Node__schema__; }

template< template < typename, int, int, int > class Member >
inline
std::string NodeTemplate< Member >::name( ) { return "Node"; }

template< template < typename, int, int, int > class Member >
inline
constexpr size_t NodeTemplate< Member >::size_in_bytes( ) { return 20; }

template< template < typename, int, int, int > class Member >
inline
bool NodeTemplate< Member >::operator==( const NodeTemplate& other ) const
{
    for ( size_t i = 0; i < size_in_bytes( ); i++ )
    {
        if ( _data.data[ i ] != other._data.data[ i ] )
        {
            return false;
        }
    }
    return true;
}

template< template < typename, int, int, int > class Member >
inline
bool NodeTemplate< Member >::operator!=( const NodeTemplate& other ) const
{
    return !( *this == other );
}

template< template < typename, int, int, int > class Member >
inline
bool NodeTemplate< Member >::operator<( const NodeTemplate& other ) const
{
return
    id < other.id &&
    lat < other.lat &&
    lon < other.lon &&
    tag_first_idx < other.tag_first_idx ;
}

template< template < typename, int, int, int > class Member >
inline
NodeTemplate< Member >::operator NodeTemplate< flatdata::Reader >( ) const
{
    return NodeTemplate< flatdata::Reader >( _data.data );
}

template< template < typename, int, int, int > class Member >
inline
std::string NodeTemplate< Member >::to_string( ) const
{
    std::ostringstream ss;
    ss << "{ " << std::endl <<
    "id : " << static_cast< uint64_t >( id ) << ", " << std::endl
    <<
    "lat : " << static_cast< uint64_t >( lat ) << ", " << std::endl
    <<
    "lon : " << static_cast< uint64_t >( lon ) << ", " << std::endl
    <<
    "tag_first_idx : " << static_cast< uint64_t >( tag_first_idx ) << ", " << std::endl
    << "}"
;
    return ss.str( );
}

template< template < typename, int, int, int > class Member >
inline
std::string NodeTemplate< Member >::describe( ) const
{
    std::ostringstream ss;
    ss << "Structure of size " << size_in_bytes( );
    return ss.str( );
}
} // namespace osm

namespace osm { 
namespace internal
{
    const char* const NodeIndex__schema__ = R"schema(namespace osm {
struct NodeIndex
{
    value : u64 : 40;
}
}

)schema";
}

template< template < typename, int, int, int > class Member >
inline
NodeIndexTemplate< Member >::NodeIndexTemplate( )
: _data( Member< uint32_t, 0, 0, 0 >{nullptr} )
{
}

template< template < typename, int, int, int > class Member >
inline
NodeIndexTemplate< Member >::NodeIndexTemplate( StreamType data )
: _data( Member< uint32_t, 0, 0, 0 >{data} )
{
}

template< template < typename, int, int, int > class Member >
inline
NodeIndexTemplate< Member >::operator bool( ) const
{
return _data.data != nullptr;
}

template< template < typename, int, int, int > class Member >
inline
typename NodeIndexTemplate< Member >::StreamType NodeIndexTemplate< Member >::data( ) const { return _data.data; }

template< template < typename, int, int, int > class Member >
inline
std::string NodeIndexTemplate< Member >::schema( ) { return internal::NodeIndex__schema__; }

template< template < typename, int, int, int > class Member >
inline
std::string NodeIndexTemplate< Member >::name( ) { return "NodeIndex"; }

template< template < typename, int, int, int > class Member >
inline
constexpr size_t NodeIndexTemplate< Member >::size_in_bytes( ) { return 5; }

template< template < typename, int, int, int > class Member >
inline
bool NodeIndexTemplate< Member >::operator==( const NodeIndexTemplate& other ) const
{
    for ( size_t i = 0; i < size_in_bytes( ); i++ )
    {
        if ( _data.data[ i ] != other._data.data[ i ] )
        {
            return false;
        }
    }
    return true;
}

template< template < typename, int, int, int > class Member >
inline
bool NodeIndexTemplate< Member >::operator!=( const NodeIndexTemplate& other ) const
{
    return !( *this == other );
}

template< template < typename, int, int, int > class Member >
inline
bool NodeIndexTemplate< Member >::operator<( const NodeIndexTemplate& other ) const
{
return
    value < other.value ;
}

template< template < typename, int, int, int > class Member >
inline
NodeIndexTemplate< Member >::operator NodeIndexTemplate< flatdata::Reader >( ) const
{
    return NodeIndexTemplate< flatdata::Reader >( _data.data );
}

template< template < typename, int, int, int > class Member >
inline
std::string NodeIndexTemplate< Member >::to_string( ) const
{
    std::ostringstream ss;
    ss << "{ " << std::endl <<
    "value : " << static_cast< uint64_t >( value ) << ", " << std::endl
    << "}"
;
    return ss.str( );
}

template< template < typename, int, int, int > class Member >
inline
std::string NodeIndexTemplate< Member >::describe( ) const
{
    std::ostringstream ss;
    ss << "Structure of size " << size_in_bytes( );
    return ss.str( );
}
} // namespace osm

namespace osm { 
namespace internal
{
    const char* const Way__schema__ = R"schema(namespace osm {
struct Way
{
    id : i64 : 40;
    @range( tags )
    tag_first_idx : u64 : 40;
    @range( refs )
    ref_first_idx : u64 : 40;
}
}

)schema";
}

template< template < typename, int, int, int > class Member >
inline
WayTemplate< Member >::WayTemplate( )
: _data( Member< uint32_t, 0, 0, 0 >{nullptr} )
{
}

template< template < typename, int, int, int > class Member >
inline
WayTemplate< Member >::WayTemplate( StreamType data )
: _data( Member< uint32_t, 0, 0, 0 >{data} )
{
}

template< template < typename, int, int, int > class Member >
inline
WayTemplate< Member >::operator bool( ) const
{
return _data.data != nullptr;
}

template< template < typename, int, int, int > class Member >
inline
typename WayTemplate< Member >::StreamType WayTemplate< Member >::data( ) const { return _data.data; }

template< template < typename, int, int, int > class Member >
inline
std::string WayTemplate< Member >::schema( ) { return internal::Way__schema__; }

template< template < typename, int, int, int > class Member >
inline
std::string WayTemplate< Member >::name( ) { return "Way"; }

template< template < typename, int, int, int > class Member >
inline
constexpr size_t WayTemplate< Member >::size_in_bytes( ) { return 15; }

template< template < typename, int, int, int > class Member >
inline
bool WayTemplate< Member >::operator==( const WayTemplate& other ) const
{
    for ( size_t i = 0; i < size_in_bytes( ); i++ )
    {
        if ( _data.data[ i ] != other._data.data[ i ] )
        {
            return false;
        }
    }
    return true;
}

template< template < typename, int, int, int > class Member >
inline
bool WayTemplate< Member >::operator!=( const WayTemplate& other ) const
{
    return !( *this == other );
}

template< template < typename, int, int, int > class Member >
inline
bool WayTemplate< Member >::operator<( const WayTemplate& other ) const
{
return
    id < other.id &&
    tag_first_idx < other.tag_first_idx &&
    ref_first_idx < other.ref_first_idx ;
}

template< template < typename, int, int, int > class Member >
inline
WayTemplate< Member >::operator WayTemplate< flatdata::Reader >( ) const
{
    return WayTemplate< flatdata::Reader >( _data.data );
}

template< template < typename, int, int, int > class Member >
inline
std::string WayTemplate< Member >::to_string( ) const
{
    std::ostringstream ss;
    ss << "{ " << std::endl <<
    "id : " << static_cast< uint64_t >( id ) << ", " << std::endl
    <<
    "tag_first_idx : " << static_cast< uint64_t >( tag_first_idx ) << ", " << std::endl
    <<
    "ref_first_idx : " << static_cast< uint64_t >( ref_first_idx ) << ", " << std::endl
    << "}"
;
    return ss.str( );
}

template< template < typename, int, int, int > class Member >
inline
std::string WayTemplate< Member >::describe( ) const
{
    std::ostringstream ss;
    ss << "Structure of size " << size_in_bytes( );
    return ss.str( );
}
} // namespace osm

namespace osm { 
namespace internal
{
    const char* const TagIndex__schema__ = R"schema(namespace osm {
struct TagIndex
{
    value : u64 : 40;
}
}

)schema";
}

template< template < typename, int, int, int > class Member >
inline
TagIndexTemplate< Member >::TagIndexTemplate( )
: _data( Member< uint32_t, 0, 0, 0 >{nullptr} )
{
}

template< template < typename, int, int, int > class Member >
inline
TagIndexTemplate< Member >::TagIndexTemplate( StreamType data )
: _data( Member< uint32_t, 0, 0, 0 >{data} )
{
}

template< template < typename, int, int, int > class Member >
inline
TagIndexTemplate< Member >::operator bool( ) const
{
return _data.data != nullptr;
}

template< template < typename, int, int, int > class Member >
inline
typename TagIndexTemplate< Member >::StreamType TagIndexTemplate< Member >::data( ) const { return _data.data; }

template< template < typename, int, int, int > class Member >
inline
std::string TagIndexTemplate< Member >::schema( ) { return internal::TagIndex__schema__; }

template< template < typename, int, int, int > class Member >
inline
std::string TagIndexTemplate< Member >::name( ) { return "TagIndex"; }

template< template < typename, int, int, int > class Member >
inline
constexpr size_t TagIndexTemplate< Member >::size_in_bytes( ) { return 5; }

template< template < typename, int, int, int > class Member >
inline
bool TagIndexTemplate< Member >::operator==( const TagIndexTemplate& other ) const
{
    for ( size_t i = 0; i < size_in_bytes( ); i++ )
    {
        if ( _data.data[ i ] != other._data.data[ i ] )
        {
            return false;
        }
    }
    return true;
}

template< template < typename, int, int, int > class Member >
inline
bool TagIndexTemplate< Member >::operator!=( const TagIndexTemplate& other ) const
{
    return !( *this == other );
}

template< template < typename, int, int, int > class Member >
inline
bool TagIndexTemplate< Member >::operator<( const TagIndexTemplate& other ) const
{
return
    value < other.value ;
}

template< template < typename, int, int, int > class Member >
inline
TagIndexTemplate< Member >::operator TagIndexTemplate< flatdata::Reader >( ) const
{
    return TagIndexTemplate< flatdata::Reader >( _data.data );
}

template< template < typename, int, int, int > class Member >
inline
std::string TagIndexTemplate< Member >::to_string( ) const
{
    std::ostringstream ss;
    ss << "{ " << std::endl <<
    "value : " << static_cast< uint64_t >( value ) << ", " << std::endl
    << "}"
;
    return ss.str( );
}

template< template < typename, int, int, int > class Member >
inline
std::string TagIndexTemplate< Member >::describe( ) const
{
    std::ostringstream ss;
    ss << "Structure of size " << size_in_bytes( );
    return ss.str( );
}
} // namespace osm

namespace osm { 
namespace internal
{
    const char* const NodeMember__schema__ = R"schema(namespace osm {
struct NodeMember
{
    node_idx : u64 : 40;
    role_idx : u64 : 40;
}
}

)schema";
}

template< template < typename, int, int, int > class Member >
inline
NodeMemberTemplate< Member >::NodeMemberTemplate( )
: _data( Member< uint32_t, 0, 0, 0 >{nullptr} )
{
}

template< template < typename, int, int, int > class Member >
inline
NodeMemberTemplate< Member >::NodeMemberTemplate( StreamType data )
: _data( Member< uint32_t, 0, 0, 0 >{data} )
{
}

template< template < typename, int, int, int > class Member >
inline
NodeMemberTemplate< Member >::operator bool( ) const
{
return _data.data != nullptr;
}

template< template < typename, int, int, int > class Member >
inline
typename NodeMemberTemplate< Member >::StreamType NodeMemberTemplate< Member >::data( ) const { return _data.data; }

template< template < typename, int, int, int > class Member >
inline
std::string NodeMemberTemplate< Member >::schema( ) { return internal::NodeMember__schema__; }

template< template < typename, int, int, int > class Member >
inline
std::string NodeMemberTemplate< Member >::name( ) { return "NodeMember"; }

template< template < typename, int, int, int > class Member >
inline
constexpr size_t NodeMemberTemplate< Member >::size_in_bytes( ) { return 10; }

template< template < typename, int, int, int > class Member >
inline
bool NodeMemberTemplate< Member >::operator==( const NodeMemberTemplate& other ) const
{
    for ( size_t i = 0; i < size_in_bytes( ); i++ )
    {
        if ( _data.data[ i ] != other._data.data[ i ] )
        {
            return false;
        }
    }
    return true;
}

template< template < typename, int, int, int > class Member >
inline
bool NodeMemberTemplate< Member >::operator!=( const NodeMemberTemplate& other ) const
{
    return !( *this == other );
}

template< template < typename, int, int, int > class Member >
inline
bool NodeMemberTemplate< Member >::operator<( const NodeMemberTemplate& other ) const
{
return
    node_idx < other.node_idx &&
    role_idx < other.role_idx ;
}

template< template < typename, int, int, int > class Member >
inline
NodeMemberTemplate< Member >::operator NodeMemberTemplate< flatdata::Reader >( ) const
{
    return NodeMemberTemplate< flatdata::Reader >( _data.data );
}

template< template < typename, int, int, int > class Member >
inline
std::string NodeMemberTemplate< Member >::to_string( ) const
{
    std::ostringstream ss;
    ss << "{ " << std::endl <<
    "node_idx : " << static_cast< uint64_t >( node_idx ) << ", " << std::endl
    <<
    "role_idx : " << static_cast< uint64_t >( role_idx ) << ", " << std::endl
    << "}"
;
    return ss.str( );
}

template< template < typename, int, int, int > class Member >
inline
std::string NodeMemberTemplate< Member >::describe( ) const
{
    std::ostringstream ss;
    ss << "Structure of size " << size_in_bytes( );
    return ss.str( );
}
} // namespace osm

namespace osm { 
namespace internal
{
    const char* const WayMember__schema__ = R"schema(namespace osm {
struct WayMember
{
    way_idx : u64 : 40;
    role_idx : u64 : 40;
}
}

)schema";
}

template< template < typename, int, int, int > class Member >
inline
WayMemberTemplate< Member >::WayMemberTemplate( )
: _data( Member< uint32_t, 0, 0, 0 >{nullptr} )
{
}

template< template < typename, int, int, int > class Member >
inline
WayMemberTemplate< Member >::WayMemberTemplate( StreamType data )
: _data( Member< uint32_t, 0, 0, 0 >{data} )
{
}

template< template < typename, int, int, int > class Member >
inline
WayMemberTemplate< Member >::operator bool( ) const
{
return _data.data != nullptr;
}

template< template < typename, int, int, int > class Member >
inline
typename WayMemberTemplate< Member >::StreamType WayMemberTemplate< Member >::data( ) const { return _data.data; }

template< template < typename, int, int, int > class Member >
inline
std::string WayMemberTemplate< Member >::schema( ) { return internal::WayMember__schema__; }

template< template < typename, int, int, int > class Member >
inline
std::string WayMemberTemplate< Member >::name( ) { return "WayMember"; }

template< template < typename, int, int, int > class Member >
inline
constexpr size_t WayMemberTemplate< Member >::size_in_bytes( ) { return 10; }

template< template < typename, int, int, int > class Member >
inline
bool WayMemberTemplate< Member >::operator==( const WayMemberTemplate& other ) const
{
    for ( size_t i = 0; i < size_in_bytes( ); i++ )
    {
        if ( _data.data[ i ] != other._data.data[ i ] )
        {
            return false;
        }
    }
    return true;
}

template< template < typename, int, int, int > class Member >
inline
bool WayMemberTemplate< Member >::operator!=( const WayMemberTemplate& other ) const
{
    return !( *this == other );
}

template< template < typename, int, int, int > class Member >
inline
bool WayMemberTemplate< Member >::operator<( const WayMemberTemplate& other ) const
{
return
    way_idx < other.way_idx &&
    role_idx < other.role_idx ;
}

template< template < typename, int, int, int > class Member >
inline
WayMemberTemplate< Member >::operator WayMemberTemplate< flatdata::Reader >( ) const
{
    return WayMemberTemplate< flatdata::Reader >( _data.data );
}

template< template < typename, int, int, int > class Member >
inline
std::string WayMemberTemplate< Member >::to_string( ) const
{
    std::ostringstream ss;
    ss << "{ " << std::endl <<
    "way_idx : " << static_cast< uint64_t >( way_idx ) << ", " << std::endl
    <<
    "role_idx : " << static_cast< uint64_t >( role_idx ) << ", " << std::endl
    << "}"
;
    return ss.str( );
}

template< template < typename, int, int, int > class Member >
inline
std::string WayMemberTemplate< Member >::describe( ) const
{
    std::ostringstream ss;
    ss << "Structure of size " << size_in_bytes( );
    return ss.str( );
}
} // namespace osm

namespace osm { 
namespace internal
{
    const char* const RelationMember__schema__ = R"schema(namespace osm {
struct RelationMember
{
    relation_idx : u64 : 40;
    role_idx : u64 : 40;
}
}

)schema";
}

template< template < typename, int, int, int > class Member >
inline
RelationMemberTemplate< Member >::RelationMemberTemplate( )
: _data( Member< uint32_t, 0, 0, 0 >{nullptr} )
{
}

template< template < typename, int, int, int > class Member >
inline
RelationMemberTemplate< Member >::RelationMemberTemplate( StreamType data )
: _data( Member< uint32_t, 0, 0, 0 >{data} )
{
}

template< template < typename, int, int, int > class Member >
inline
RelationMemberTemplate< Member >::operator bool( ) const
{
return _data.data != nullptr;
}

template< template < typename, int, int, int > class Member >
inline
typename RelationMemberTemplate< Member >::StreamType RelationMemberTemplate< Member >::data( ) const { return _data.data; }

template< template < typename, int, int, int > class Member >
inline
std::string RelationMemberTemplate< Member >::schema( ) { return internal::RelationMember__schema__; }

template< template < typename, int, int, int > class Member >
inline
std::string RelationMemberTemplate< Member >::name( ) { return "RelationMember"; }

template< template < typename, int, int, int > class Member >
inline
constexpr size_t RelationMemberTemplate< Member >::size_in_bytes( ) { return 10; }

template< template < typename, int, int, int > class Member >
inline
bool RelationMemberTemplate< Member >::operator==( const RelationMemberTemplate& other ) const
{
    for ( size_t i = 0; i < size_in_bytes( ); i++ )
    {
        if ( _data.data[ i ] != other._data.data[ i ] )
        {
            return false;
        }
    }
    return true;
}

template< template < typename, int, int, int > class Member >
inline
bool RelationMemberTemplate< Member >::operator!=( const RelationMemberTemplate& other ) const
{
    return !( *this == other );
}

template< template < typename, int, int, int > class Member >
inline
bool RelationMemberTemplate< Member >::operator<( const RelationMemberTemplate& other ) const
{
return
    relation_idx < other.relation_idx &&
    role_idx < other.role_idx ;
}

template< template < typename, int, int, int > class Member >
inline
RelationMemberTemplate< Member >::operator RelationMemberTemplate< flatdata::Reader >( ) const
{
    return RelationMemberTemplate< flatdata::Reader >( _data.data );
}

template< template < typename, int, int, int > class Member >
inline
std::string RelationMemberTemplate< Member >::to_string( ) const
{
    std::ostringstream ss;
    ss << "{ " << std::endl <<
    "relation_idx : " << static_cast< uint64_t >( relation_idx ) << ", " << std::endl
    <<
    "role_idx : " << static_cast< uint64_t >( role_idx ) << ", " << std::endl
    << "}"
;
    return ss.str( );
}

template< template < typename, int, int, int > class Member >
inline
std::string RelationMemberTemplate< Member >::describe( ) const
{
    std::ostringstream ss;
    ss << "Structure of size " << size_in_bytes( );
    return ss.str( );
}
} // namespace osm

namespace osm { 
namespace internal
{
    const char* const Relation__schema__ = R"schema(namespace osm {
struct Relation
{
    id : i64 : 40;
    @range( tags )
    tag_first_idx : u64 : 40;
}
}

)schema";
}

template< template < typename, int, int, int > class Member >
inline
RelationTemplate< Member >::RelationTemplate( )
: _data( Member< uint32_t, 0, 0, 0 >{nullptr} )
{
}

template< template < typename, int, int, int > class Member >
inline
RelationTemplate< Member >::RelationTemplate( StreamType data )
: _data( Member< uint32_t, 0, 0, 0 >{data} )
{
}

template< template < typename, int, int, int > class Member >
inline
RelationTemplate< Member >::operator bool( ) const
{
return _data.data != nullptr;
}

template< template < typename, int, int, int > class Member >
inline
typename RelationTemplate< Member >::StreamType RelationTemplate< Member >::data( ) const { return _data.data; }

template< template < typename, int, int, int > class Member >
inline
std::string RelationTemplate< Member >::schema( ) { return internal::Relation__schema__; }

template< template < typename, int, int, int > class Member >
inline
std::string RelationTemplate< Member >::name( ) { return "Relation"; }

template< template < typename, int, int, int > class Member >
inline
constexpr size_t RelationTemplate< Member >::size_in_bytes( ) { return 10; }

template< template < typename, int, int, int > class Member >
inline
bool RelationTemplate< Member >::operator==( const RelationTemplate& other ) const
{
    for ( size_t i = 0; i < size_in_bytes( ); i++ )
    {
        if ( _data.data[ i ] != other._data.data[ i ] )
        {
            return false;
        }
    }
    return true;
}

template< template < typename, int, int, int > class Member >
inline
bool RelationTemplate< Member >::operator!=( const RelationTemplate& other ) const
{
    return !( *this == other );
}

template< template < typename, int, int, int > class Member >
inline
bool RelationTemplate< Member >::operator<( const RelationTemplate& other ) const
{
return
    id < other.id &&
    tag_first_idx < other.tag_first_idx ;
}

template< template < typename, int, int, int > class Member >
inline
RelationTemplate< Member >::operator RelationTemplate< flatdata::Reader >( ) const
{
    return RelationTemplate< flatdata::Reader >( _data.data );
}

template< template < typename, int, int, int > class Member >
inline
std::string RelationTemplate< Member >::to_string( ) const
{
    std::ostringstream ss;
    ss << "{ " << std::endl <<
    "id : " << static_cast< uint64_t >( id ) << ", " << std::endl
    <<
    "tag_first_idx : " << static_cast< uint64_t >( tag_first_idx ) << ", " << std::endl
    << "}"
;
    return ss.str( );
}

template< template < typename, int, int, int > class Member >
inline
std::string RelationTemplate< Member >::describe( ) const
{
    std::ostringstream ss;
    ss << "Structure of size " << size_in_bytes( );
    return ss.str( );
}
} // namespace osm

namespace _builtin { namespace multivector { 
namespace internal
{
    const char* const IndexType40__schema__ = R"schema()schema";
}

template< template < typename, int, int, int > class Member >
inline
IndexType40Template< Member >::IndexType40Template( )
: _data( Member< uint32_t, 0, 0, 0 >{nullptr} )
{
}

template< template < typename, int, int, int > class Member >
inline
IndexType40Template< Member >::IndexType40Template( StreamType data )
: _data( Member< uint32_t, 0, 0, 0 >{data} )
{
}

template< template < typename, int, int, int > class Member >
inline
IndexType40Template< Member >::operator bool( ) const
{
return _data.data != nullptr;
}

template< template < typename, int, int, int > class Member >
inline
typename IndexType40Template< Member >::StreamType IndexType40Template< Member >::data( ) const { return _data.data; }

template< template < typename, int, int, int > class Member >
inline
std::string IndexType40Template< Member >::schema( ) { return internal::IndexType40__schema__; }

template< template < typename, int, int, int > class Member >
inline
std::string IndexType40Template< Member >::name( ) { return "IndexType40"; }

template< template < typename, int, int, int > class Member >
inline
constexpr size_t IndexType40Template< Member >::size_in_bytes( ) { return 5; }

template< template < typename, int, int, int > class Member >
inline
bool IndexType40Template< Member >::operator==( const IndexType40Template& other ) const
{
    for ( size_t i = 0; i < size_in_bytes( ); i++ )
    {
        if ( _data.data[ i ] != other._data.data[ i ] )
        {
            return false;
        }
    }
    return true;
}

template< template < typename, int, int, int > class Member >
inline
bool IndexType40Template< Member >::operator!=( const IndexType40Template& other ) const
{
    return !( *this == other );
}

template< template < typename, int, int, int > class Member >
inline
bool IndexType40Template< Member >::operator<( const IndexType40Template& other ) const
{
return
    value < other.value ;
}

template< template < typename, int, int, int > class Member >
inline
IndexType40Template< Member >::operator IndexType40Template< flatdata::Reader >( ) const
{
    return IndexType40Template< flatdata::Reader >( _data.data );
}

template< template < typename, int, int, int > class Member >
inline
std::string IndexType40Template< Member >::to_string( ) const
{
    std::ostringstream ss;
    ss << "{ " << std::endl <<
    "value : " << static_cast< uint64_t >( value ) << ", " << std::endl
    << "}"
;
    return ss.str( );
}

template< template < typename, int, int, int > class Member >
inline
std::string IndexType40Template< Member >::describe( ) const
{
    std::ostringstream ss;
    ss << "Structure of size " << size_in_bytes( );
    return ss.str( );
}
}} // namespace _builtin.multivector

namespace osm { 
namespace internal
{
const char* const Osm__schema__ =
"namespace osm {\n"
    "struct Header\n"
    "{\n"
    "    bbox_left : i64 : 40;\n"
    "    bbox_right : i64 : 40;\n"
    "    bbox_top : i64 : 40;\n"
    "    bbox_bottom : i64 : 40;\n"
    "    required_feature_first_idx : u64 : 40;\n"
    "    required_features_size : u32 : 4;\n"
    "    optional_feature_first_idx : u64 : 40;\n"
    "    optional_features_size : u32 : 4;\n"
    "    writingprogram_idx : u64 : 40;\n"
    "    source_idx : u64 : 40;\n"
    "    osmosis_replication_timestamp : i64 : 64;\n"
    "    osmosis_replication_sequence_number : i64 : 64;\n"
    "    osmosis_replication_base_url_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "struct Node\n"
    "{\n"
    "    id : i64 : 40;\n"
    "    lat : i64 : 40;\n"
    "    lon : i64 : 40;\n"
    "    @range( tags )\n"
    "    tag_first_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "struct Way\n"
    "{\n"
    "    id : i64 : 40;\n"
    "    @range( tags )\n"
    "    tag_first_idx : u64 : 40;\n"
    "    @range( refs )\n"
    "    ref_first_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "struct Relation\n"
    "{\n"
    "    id : i64 : 40;\n"
    "    @range( tags )\n"
    "    tag_first_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "struct NodeMember\n"
    "{\n"
    "    node_idx : u64 : 40;\n"
    "    role_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "struct WayMember\n"
    "{\n"
    "    way_idx : u64 : 40;\n"
    "    role_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "struct RelationMember\n"
    "{\n"
    "    relation_idx : u64 : 40;\n"
    "    role_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "struct Tag\n"
    "{\n"
    "    key_idx : u64 : 40;\n"
    "    value_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "struct TagIndex\n"
    "{\n"
    "    value : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "struct NodeIndex\n"
    "{\n"
    "    value : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "const u64 INVALID_IDX = 1099511627775;\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "const u64 COORD_SCALE = 1000000000;\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "@bound_implicitly( Relations : .osm.Osm.relations, .osm.Osm.relation_members )\n"
    "archive Osm\n"
    "{\n"
    "    @explicit_reference( .osm.Header.required_feature_first_idx, .osm.Osm.stringtable )\n"
    "    @explicit_reference( .osm.Header.optional_feature_first_idx, .osm.Osm.stringtable )\n"
    "    @explicit_reference( .osm.Header.writingprogram_idx, .osm.Osm.stringtable )\n"
    "    @explicit_reference( .osm.Header.source_idx, .osm.Osm.stringtable )\n"
    "    @explicit_reference( .osm.Header.osmosis_replication_base_url_idx, .osm.Osm.stringtable )\n"
    "    header : .osm.Header;\n"
    "    @explicit_reference( .osm.Node.tag_first_idx, .osm.Osm.tags_index )\n"
    "    nodes : vector< .osm.Node >;\n"
    "    @explicit_reference( .osm.Way.tag_first_idx, .osm.Osm.tags_index )\n"
    "    @explicit_reference( .osm.Way.ref_first_idx, .osm.Osm.nodes_index )\n"
    "    ways : vector< .osm.Way >;\n"
    "    @explicit_reference( .osm.Relation.tag_first_idx, .osm.Osm.tags_index )\n"
    "    relations : vector< .osm.Relation >;\n"
    "    @explicit_reference( .osm.NodeMember.node_idx, .osm.Osm.nodes )\n"
    "    @explicit_reference( .osm.NodeMember.role_idx, .osm.Osm.stringtable )\n"
    "    @explicit_reference( .osm.WayMember.way_idx, .osm.Osm.ways )\n"
    "    @explicit_reference( .osm.WayMember.role_idx, .osm.Osm.stringtable )\n"
    "    @explicit_reference( .osm.RelationMember.relation_idx, .osm.Osm.relations )\n"
    "    @explicit_reference( .osm.RelationMember.role_idx, .osm.Osm.stringtable )\n"
    "    relation_members : multivector< 40, .osm.NodeMember, .osm.WayMember, .osm.RelationMember >;\n"
    "    @explicit_reference( .osm.Tag.key_idx, .osm.Osm.stringtable )\n"
    "    @explicit_reference( .osm.Tag.value_idx, .osm.Osm.stringtable )\n"
    "    tags : vector< .osm.Tag >;\n"
    "    @explicit_reference( .osm.TagIndex.value, .osm.Osm.tags )\n"
    "    tags_index : vector< .osm.TagIndex >;\n"
    "    @explicit_reference( .osm.NodeIndex.value, .osm.Osm.nodes )\n"
    "    nodes_index : vector< .osm.NodeIndex >;\n"
    "    stringtable : raw_data;\n"
    "}\n"
    "}\n"
    "\n"
    "";
const char* const Osm__header__schema__ =
"namespace osm {\n"
    "struct Header\n"
    "{\n"
    "    bbox_left : i64 : 40;\n"
    "    bbox_right : i64 : 40;\n"
    "    bbox_top : i64 : 40;\n"
    "    bbox_bottom : i64 : 40;\n"
    "    required_feature_first_idx : u64 : 40;\n"
    "    required_features_size : u32 : 4;\n"
    "    optional_feature_first_idx : u64 : 40;\n"
    "    optional_features_size : u32 : 4;\n"
    "    writingprogram_idx : u64 : 40;\n"
    "    source_idx : u64 : 40;\n"
    "    osmosis_replication_timestamp : i64 : 64;\n"
    "    osmosis_replication_sequence_number : i64 : 64;\n"
    "    osmosis_replication_base_url_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "archive Osm\n"
    "{\n"
    "    @explicit_reference( .osm.Header.required_feature_first_idx, .osm.Osm.stringtable )\n"
    "    @explicit_reference( .osm.Header.optional_feature_first_idx, .osm.Osm.stringtable )\n"
    "    @explicit_reference( .osm.Header.writingprogram_idx, .osm.Osm.stringtable )\n"
    "    @explicit_reference( .osm.Header.source_idx, .osm.Osm.stringtable )\n"
    "    @explicit_reference( .osm.Header.osmosis_replication_base_url_idx, .osm.Osm.stringtable )\n"
    "    header : .osm.Header;\n"
    "}\n"
    "}\n"
    "\n"
    "";
const char* const Osm__nodes__schema__ =
"namespace osm {\n"
    "struct Node\n"
    "{\n"
    "    id : i64 : 40;\n"
    "    lat : i64 : 40;\n"
    "    lon : i64 : 40;\n"
    "    @range( tags )\n"
    "    tag_first_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "archive Osm\n"
    "{\n"
    "    @explicit_reference( .osm.Node.tag_first_idx, .osm.Osm.tags_index )\n"
    "    nodes : vector< .osm.Node >;\n"
    "}\n"
    "}\n"
    "\n"
    "";
const char* const Osm__ways__schema__ =
"namespace osm {\n"
    "struct Way\n"
    "{\n"
    "    id : i64 : 40;\n"
    "    @range( tags )\n"
    "    tag_first_idx : u64 : 40;\n"
    "    @range( refs )\n"
    "    ref_first_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "archive Osm\n"
    "{\n"
    "    @explicit_reference( .osm.Way.tag_first_idx, .osm.Osm.tags_index )\n"
    "    @explicit_reference( .osm.Way.ref_first_idx, .osm.Osm.nodes_index )\n"
    "    ways : vector< .osm.Way >;\n"
    "}\n"
    "}\n"
    "\n"
    "";
const char* const Osm__relations__schema__ =
"namespace osm {\n"
    "struct Relation\n"
    "{\n"
    "    id : i64 : 40;\n"
    "    @range( tags )\n"
    "    tag_first_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "archive Osm\n"
    "{\n"
    "    @explicit_reference( .osm.Relation.tag_first_idx, .osm.Osm.tags_index )\n"
    "    relations : vector< .osm.Relation >;\n"
    "}\n"
    "}\n"
    "\n"
    "";
const char* const Osm__relation_members__schema__ =
"namespace osm {\n"
    "struct NodeMember\n"
    "{\n"
    "    node_idx : u64 : 40;\n"
    "    role_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "struct WayMember\n"
    "{\n"
    "    way_idx : u64 : 40;\n"
    "    role_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "struct RelationMember\n"
    "{\n"
    "    relation_idx : u64 : 40;\n"
    "    role_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "archive Osm\n"
    "{\n"
    "    @explicit_reference( .osm.NodeMember.node_idx, .osm.Osm.nodes )\n"
    "    @explicit_reference( .osm.NodeMember.role_idx, .osm.Osm.stringtable )\n"
    "    @explicit_reference( .osm.WayMember.way_idx, .osm.Osm.ways )\n"
    "    @explicit_reference( .osm.WayMember.role_idx, .osm.Osm.stringtable )\n"
    "    @explicit_reference( .osm.RelationMember.relation_idx, .osm.Osm.relations )\n"
    "    @explicit_reference( .osm.RelationMember.role_idx, .osm.Osm.stringtable )\n"
    "    relation_members : multivector< 40, .osm.NodeMember, .osm.WayMember, .osm.RelationMember >;\n"
    "}\n"
    "}\n"
    "\n"
    "";
const char* const Osm__tags__schema__ =
"namespace osm {\n"
    "struct Tag\n"
    "{\n"
    "    key_idx : u64 : 40;\n"
    "    value_idx : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "archive Osm\n"
    "{\n"
    "    @explicit_reference( .osm.Tag.key_idx, .osm.Osm.stringtable )\n"
    "    @explicit_reference( .osm.Tag.value_idx, .osm.Osm.stringtable )\n"
    "    tags : vector< .osm.Tag >;\n"
    "}\n"
    "}\n"
    "\n"
    "";
const char* const Osm__tags_index__schema__ =
"namespace osm {\n"
    "struct TagIndex\n"
    "{\n"
    "    value : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "archive Osm\n"
    "{\n"
    "    @explicit_reference( .osm.TagIndex.value, .osm.Osm.tags )\n"
    "    tags_index : vector< .osm.TagIndex >;\n"
    "}\n"
    "}\n"
    "\n"
    "";
const char* const Osm__nodes_index__schema__ =
"namespace osm {\n"
    "struct NodeIndex\n"
    "{\n"
    "    value : u64 : 40;\n"
    "}\n"
    "}\n"
    "\n"
    "namespace osm {\n"
    "archive Osm\n"
    "{\n"
    "    @explicit_reference( .osm.NodeIndex.value, .osm.Osm.nodes )\n"
    "    nodes_index : vector< .osm.NodeIndex >;\n"
    "}\n"
    "}\n"
    "\n"
    "";
const char* const Osm__stringtable__schema__ =
"namespace osm {\n"
    "archive Osm\n"
    "{\n"
    "    stringtable : raw_data;\n"
    "}\n"
    "}\n"
    "\n"
    "";
}
// -------------------------------------------------------------------------------------------------

inline const char*
Osm::schema_definition( )
{
    return internal::Osm__schema__;
}

inline const char*
Osm::name_definition( )
{
    return "Osm";
}

inline const char*
Osm::name( ) const
{
    return Osm::name_definition( );
}

inline const char*
Osm::schema( ) const
{
    return Osm::schema_definition( );
}

inline
Osm
Osm::open( std::shared_ptr< flatdata::ResourceStorage > storage )
{
    Osm result( storage );
    result.initialize( );
    return result;
}

inline
Osm::Osm( std::shared_ptr< flatdata::ResourceStorage > storage )
    : flatdata::Archive( storage )
{
}

inline bool
Osm::load_contents( )
{
    bool is_open = true;

    read_resource( is_open, m_header, "header", internal::Osm__header__schema__ );
    read_resource( is_open, m_nodes, "nodes", internal::Osm__nodes__schema__ );
    read_resource( is_open, m_ways, "ways", internal::Osm__ways__schema__ );
    read_resource( is_open, m_relations, "relations", internal::Osm__relations__schema__ );
    read_resource( is_open, m_relation_members, "relation_members", internal::Osm__relation_members__schema__ );
    read_resource( is_open, m_tags, "tags", internal::Osm__tags__schema__ );
    read_resource( is_open, m_tags_index, "tags_index", internal::Osm__tags_index__schema__ );
    read_resource( is_open, m_nodes_index, "nodes_index", internal::Osm__nodes_index__schema__ );
    read_resource( is_open, m_stringtable, "stringtable", internal::Osm__stringtable__schema__ );
    return is_open;
}

inline void
Osm::describe_resources( std::ostream& stream ) const
{
    describe_resource( stream, "header", m_header );
    describe_resource( stream, "nodes", m_nodes );
    describe_resource( stream, "ways", m_ways );
    describe_resource( stream, "relations", m_relations );
    describe_resource( stream, "relation_members", m_relation_members );
    describe_resource( stream, "tags", m_tags );
    describe_resource( stream, "tags_index", m_tags_index );
    describe_resource( stream, "nodes_index", m_nodes_index );
    describe_resource( stream, "stringtable", m_stringtable );
}

inline auto Osm::header( ) const -> const HeaderType&
{
    return m_header;
}

inline auto Osm::nodes( ) const -> const NodesType&
{
    return m_nodes;
}

inline auto Osm::ways( ) const -> const WaysType&
{
    return m_ways;
}

inline auto Osm::relations( ) const -> const RelationsType&
{
    return m_relations;
}

inline auto Osm::relation_members( ) const -> const RelationMembersType&
{
    return m_relation_members;
}

inline auto Osm::tags( ) const -> const TagsType&
{
    return m_tags;
}

inline auto Osm::tags_index( ) const -> const TagsIndexType&
{
    return m_tags_index;
}

inline auto Osm::nodes_index( ) const -> const NodesIndexType&
{
    return m_nodes_index;
}

inline auto Osm::stringtable( ) const -> const StringtableType&
{
    return m_stringtable;
}


// -------------------------------------------------------------------------------------------------

inline const char*
OsmBuilder::schema_definition( )
{
    return internal::Osm__schema__;
}

inline const char*
OsmBuilder::name( ) const
{
    return "Osm";
}

inline const char*
OsmBuilder::schema( ) const
{
    return Osm::schema_definition( );
}

inline
OsmBuilder::OsmBuilder( std::shared_ptr< flatdata::ResourceStorage > storage )
    : flatdata::ArchiveBuilder( storage )
{
}


inline OsmBuilder
OsmBuilder::open(std::shared_ptr< flatdata::ResourceStorage > storage )
{
    OsmBuilder result( storage );
    if ( !result.initialize( ) )
    {
        return OsmBuilder( );
    }
    return result;
}

inline bool
OsmBuilder::set_header( HeaderReaderType data )
{
    check_created( );
    return storage( ).write< HeaderReaderType >( "header", internal::Osm__header__schema__, data );
}
inline auto OsmBuilder::start_nodes( ) -> NodesType
{
    check_created( );
    return storage( ).create_external_vector< ::osm::Node >( "nodes", internal::Osm__nodes__schema__ );
}
inline bool
OsmBuilder::set_nodes( NodesReaderType data )
{
    check_created( );
    return storage( ).write< NodesReaderType >( "nodes", internal::Osm__nodes__schema__, data );
}
inline auto OsmBuilder::start_ways( ) -> WaysType
{
    check_created( );
    return storage( ).create_external_vector< ::osm::Way >( "ways", internal::Osm__ways__schema__ );
}
inline bool
OsmBuilder::set_ways( WaysReaderType data )
{
    check_created( );
    return storage( ).write< WaysReaderType >( "ways", internal::Osm__ways__schema__, data );
}
inline auto OsmBuilder::start_relations( ) -> RelationsType
{
    check_created( );
    return storage( ).create_external_vector< ::osm::Relation >( "relations", internal::Osm__relations__schema__ );
}
inline bool
OsmBuilder::set_relations( RelationsReaderType data )
{
    check_created( );
    return storage( ).write< RelationsReaderType >( "relations", internal::Osm__relations__schema__, data );
}
inline auto OsmBuilder::start_relation_members( ) -> RelationMembersType
{
    check_created( );
    return storage( ).create_multi_vector< ::_builtin::multivector::IndexType40, ::osm::NodeMember, ::osm::WayMember, ::osm::RelationMember >( "relation_members", internal::Osm__relation_members__schema__ );
}
inline auto OsmBuilder::start_tags( ) -> TagsType
{
    check_created( );
    return storage( ).create_external_vector< ::osm::Tag >( "tags", internal::Osm__tags__schema__ );
}
inline bool
OsmBuilder::set_tags( TagsReaderType data )
{
    check_created( );
    return storage( ).write< TagsReaderType >( "tags", internal::Osm__tags__schema__, data );
}
inline auto OsmBuilder::start_tags_index( ) -> TagsIndexType
{
    check_created( );
    return storage( ).create_external_vector< ::osm::TagIndex >( "tags_index", internal::Osm__tags_index__schema__ );
}
inline bool
OsmBuilder::set_tags_index( TagsIndexReaderType data )
{
    check_created( );
    return storage( ).write< TagsIndexReaderType >( "tags_index", internal::Osm__tags_index__schema__, data );
}
inline auto OsmBuilder::start_nodes_index( ) -> NodesIndexType
{
    check_created( );
    return storage( ).create_external_vector< ::osm::NodeIndex >( "nodes_index", internal::Osm__nodes_index__schema__ );
}
inline bool
OsmBuilder::set_nodes_index( NodesIndexReaderType data )
{
    check_created( );
    return storage( ).write< NodesIndexReaderType >( "nodes_index", internal::Osm__nodes_index__schema__, data );
}
inline bool
OsmBuilder::set_stringtable( StringtableReaderType data )
{
    check_created( );
    return storage( ).write< StringtableReaderType >( "stringtable", internal::Osm__stringtable__schema__, data );
}

} // namespace osm

