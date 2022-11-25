use http::{Method, Request};
use hyper::Body;
use serde::{
    de::{DeserializeOwned, IgnoredAny},
    Deserialize, Deserializer, Serialize,
};
use time::OffsetDateTime;
use url::Url;

use crate::{Error, SpotifyId};

use super::SpClient;

pub trait GraphQlRequest {
    type Variables: Serialize;
    type Extensions: Serialize;
    type Response: DeserializeOwned;

    fn operation_name(&self) -> &str;
    fn variables(&self) -> &Self::Variables;
    fn extensions(&self) -> Self::Extensions;
}

#[derive(Debug, Serialize)]
pub struct PersistedQuery {
    #[serde(rename = "persistedQuery")]
    inner: PersistedQueryInner,
}

#[derive(Debug, Serialize)]
struct PersistedQueryInner {
    version: u32,
    #[serde(rename = "sha256Hash")]
    sha256_hash: &'static str,
}

impl PersistedQuery {
    pub fn new(version: u32, sha256_hash: &'static str) -> Self {
        PersistedQuery {
            inner: PersistedQueryInner {
                version,
                sha256_hash,
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OffsetLimit {
    pub offset: u32,
    pub limit: u32,
}

impl SpClient {
    pub async fn graphql_request<R: GraphQlRequest>(
        &self,
        request: &R,
    ) -> Result<R::Response, Error> {
        let mut url = Url::parse("https://api-partner.spotify.com/pathfinder/v1/query").unwrap();

        url.query_pairs_mut()
            .append_pair("operationName", request.operation_name())
            .append_pair("variables", &serde_json::to_string(request.variables())?)
            .append_pair("extensions", &serde_json::to_string(&request.extensions())?);

        let mut request = Request::builder()
            .method(Method::POST)
            .uri(url.as_str())
            .body(Body::empty())?;
        self.add_request_headers(request.headers_mut()).await?;

        let response_bytes = self.session().http_client().request_body(request).await?;
        let response: GraphQlResponse<R::Response> = serde_json::from_slice(&response_bytes)?;
        Ok(response.data)
    }

    pub async fn get_library_albums(
        &self,
        offset_limit: OffsetLimit,
    ) -> Result<PageResponse<LibraryAlbumResponse>, Error> {
        self.graphql_request(&LibraryAlbumsRequest(offset_limit))
            .await
            .map(|data| data.me.library.albums)
    }
}

#[derive(Debug, Deserialize)]
struct GraphQlResponse<R> {
    data: R,
    extensions: IgnoredAny,
}

#[derive(Debug, Deserialize)]
pub struct MeResponse<R> {
    pub me: R,
}

#[derive(Debug, Deserialize)]
pub struct LibraryResponse<R> {
    pub library: R,
}

#[derive(Debug, Deserialize)]
pub struct ItemsResponse<I> {
    pub items: Vec<I>,
}

#[derive(Debug, Deserialize)]
pub struct PageResponse<I> {
    items: Vec<I>,
    #[serde(rename = "pagingInfo")]
    paging_info: OffsetLimit,
    #[serde(rename = "totalCount")]
    total_count: u64,
}

struct LibraryAlbumsRequest(OffsetLimit);

impl GraphQlRequest for LibraryAlbumsRequest {
    type Variables = OffsetLimit;
    type Extensions = PersistedQuery;
    type Response = MeResponse<LibraryResponse<AlbumsResponse<PageResponse<LibraryAlbumResponse>>>>;

    fn operation_name(&self) -> &str {
        "fetchLibraryAlbums"
    }

    fn variables(&self) -> &Self::Variables {
        &self.0
    }

    fn extensions(&self) -> Self::Extensions {
        PersistedQuery::new(
            1,
            "e18c65b7c99cd9c92545c6aa7d463170760bed0123ac01d85caca1fc3ff2ab67",
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct AlbumsResponse<R> {
    pub albums: R,
}

#[derive(Debug, Deserialize)]
pub struct LibraryAlbumResponse {
    #[serde(rename = "addedAt")]
    #[serde(deserialize_with = "deserialize_iso_string")]
    added_at: OffsetDateTime,
    pub album: LibraryAlbumResponseAlbum,
}

#[derive(Debug, Deserialize)]
pub struct LibraryAlbumResponseAlbum {
    #[serde(rename = "_uri")]
    pub uri: SpotifyId,
    pub data: LibraryAlbumResponseAlbumData,
}

#[derive(Debug, Deserialize)]
pub struct LibraryAlbumResponseAlbumData {
    pub name: String,
    pub artists: ItemsResponse<LibraryAlbumResponseAlbumDataArtist>,
    #[serde(rename = "coverArt")]
    pub cover_art: LibraryAlbumResponseAlbumDataCoverArt,
    #[serde(deserialize_with = "deserialize_iso_string")]
    date: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
pub struct LibraryAlbumResponseAlbumDataArtist {
    pub uri: SpotifyId,
    pub profile: LibraryAlbumResponseAlbumDataArtistProfile,
}

#[derive(Debug, Deserialize)]
pub struct LibraryAlbumResponseAlbumDataArtistProfile {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct LibraryAlbumResponseAlbumDataCoverArt {
    pub sources: Vec<LibraryAlbumResponseAlbumDataCoverArtSource>,
}

#[derive(Debug, Deserialize)]
pub struct LibraryAlbumResponseAlbumDataCoverArtSource {
    pub url: String,
    pub width: u16,
    pub height: u16,
}

/// Deserializes an object like `{isoString: "2020-11-07T03:27:58Z"}` in to a `OffsetDateTime`
fn deserialize_iso_string<'de, D>(deserializer: D) -> Result<OffsetDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct IsoStringWrapper {
        #[serde(rename = "isoString")]
        #[serde(deserialize_with = "time::serde::iso8601::deserialize")]
        iso_string: OffsetDateTime,
    }

    IsoStringWrapper::deserialize(deserializer).map(|wrapper| wrapper.iso_string)
}
