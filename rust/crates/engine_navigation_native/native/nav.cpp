// Private checked C ABI over the frozen Recast/Detour 1.6.0 sources.
#include <Recast.h>
#include <DetourNavMesh.h>
#include <DetourNavMeshBuilder.h>
#include <DetourNavMeshQuery.h>
#include <DetourCommon.h>
#include <DetourAlloc.h>
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <memory>
#include <vector>
static_assert(sizeof(dtPolyRef)==8,"371 requires DT_POLYREF64");
struct BakeConfig {float cs,ch,radius,height,climb,slope,width,bmin[3],bmax[3];int x,z;};
struct Link {float start[3],end[3],radius;uint32_t bidirectional,id;};
struct NativeNav {dtNavMesh* mesh=nullptr;dtNavMeshQuery* query=nullptr;~NativeNav(){dtFreeNavMeshQuery(query);dtFreeNavMesh(mesh);}};
static bool finite3(const float* p){return std::isfinite(p[0])&&std::isfinite(p[1])&&std::isfinite(p[2]);}
static bool validate_tile(const unsigned char* bytes,int size){
 if(!bytes||size<(int)sizeof(dtMeshHeader)||size>16*1024*1024)return false;
 dtMeshHeader h;std::memcpy(&h,bytes,sizeof(h));
 if(h.magic!=DT_NAVMESH_MAGIC||h.version!=DT_NAVMESH_VERSION||h.layer!=0||h.polyCount<1||h.polyCount>8192||h.vertCount<3||h.vertCount>65535||h.maxLinkCount<0||h.maxLinkCount>262144||h.detailMeshCount<0||h.detailMeshCount>h.polyCount||h.detailVertCount<0||h.detailVertCount>65535||h.detailTriCount<0||h.detailTriCount>262144||h.bvNodeCount<0||h.bvNodeCount>2*h.polyCount||h.offMeshConCount<0||h.offMeshConCount>1024||h.offMeshBase<0||h.offMeshBase>h.polyCount||h.offMeshBase+h.offMeshConCount!=h.polyCount||h.detailMeshCount!=h.offMeshBase||!finite3(h.bmin)||!finite3(h.bmax)||!std::isfinite(h.bvQuantFactor)||h.bvQuantFactor<=0)return false;
 const int hs=dtAlign4(sizeof(dtMeshHeader)),vs=dtAlign4(sizeof(float)*3*h.vertCount),ps=dtAlign4(sizeof(dtPoly)*h.polyCount),ls=dtAlign4(sizeof(dtLink)*h.maxLinkCount),ds=dtAlign4(sizeof(dtPolyDetail)*h.detailMeshCount),dvs=dtAlign4(sizeof(float)*3*h.detailVertCount),dts=dtAlign4(4*h.detailTriCount),bs=dtAlign4(sizeof(dtBVNode)*h.bvNodeCount),os=dtAlign4(sizeof(dtOffMeshConnection)*h.offMeshConCount);
 if(hs+vs+ps+ls+ds+dvs+dts+bs+os!=size)return false;
 const float* verts=(const float*)(bytes+hs);const dtPoly* polys=(const dtPoly*)(bytes+hs+vs);const dtPolyDetail* details=(const dtPolyDetail*)(bytes+hs+vs+ps+ls);const float* dv=(const float*)(bytes+hs+vs+ps+ls+ds);const unsigned char* tris=bytes+hs+vs+ps+ls+ds+dvs;const dtBVNode* bv=(const dtBVNode*)(tris+dts);const dtOffMeshConnection* off=(const dtOffMeshConnection*)((const unsigned char*)bv+bs);
 for(int i=0;i<h.vertCount*3;i++)if(!std::isfinite(verts[i])||std::abs(verts[i])>32768)return false;
 for(int i=0;i<h.detailVertCount*3;i++)if(!std::isfinite(dv[i])||std::abs(dv[i])>32768)return false;
 for(int i=0;i<h.polyCount;i++){const auto& p=polys[i];if(p.vertCount<2||p.vertCount>DT_VERTS_PER_POLYGON||p.getType()!=(i<h.offMeshBase?DT_POLYTYPE_GROUND:DT_POLYTYPE_OFFMESH_CONNECTION)||(i<h.offMeshBase&&p.vertCount<3))return false;for(int j=0;j<p.vertCount;j++)if(p.verts[j]>=h.vertCount||(!(p.neis[j]&DT_EXT_LINK)&&p.neis[j]>h.polyCount))return false;}
 for(int i=0;i<h.detailMeshCount;i++){const auto& d=details[i];if(d.vertBase+d.vertCount>(unsigned)h.detailVertCount||d.triBase+d.triCount>(unsigned)h.detailTriCount)return false;for(unsigned t=d.triBase;t<d.triBase+d.triCount;t++)for(int j=0;j<3;j++)if(tris[t*4+j]>=polys[i].vertCount+d.vertCount)return false;}
 for(int i=0;i<h.bvNodeCount;i++)if(bv[i].i>=h.polyCount||(bv[i].i<0&&-(int64_t)bv[i].i>h.bvNodeCount-i))return false;
 for(int i=0;i<h.offMeshConCount;i++)if(off[i].poly!=h.offMeshBase+i||!finite3(off[i].pos)||!finite3(off[i].pos+3)||!std::isfinite(off[i].rad)||off[i].rad<=0)return false;
 return true;
}
extern "C" {
int aife_nav_ref_bits(){return sizeof(dtPolyRef)*8;}
void aife_nav_free_bytes(unsigned char* p){dtFree(p);}
int aife_nav_validate(const unsigned char* p,int n){if(!p||n<1||n>16*1024*1024)return 1;std::vector<unsigned char> copy(p,p+n);return validate_tile(copy.data(),n)?0:1;}
int aife_nav_translate(unsigned char* bytes,int size,const float* delta,int dx,int dz){
 if(!validate_tile(bytes,size)||!delta||!finite3(delta))return 1;
 auto h=(dtMeshHeader*)bytes;const int64_t x=(int64_t)h->x+dx,z=(int64_t)h->y+dz;if(x<INT32_MIN||x>INT32_MAX||z<INT32_MIN||z>INT32_MAX)return 1;h->x=(int)x;h->y=(int)z;for(int j=0;j<3;j++){h->bmin[j]+=delta[j];h->bmax[j]+=delta[j];}
 const int hs=dtAlign4(sizeof(dtMeshHeader)),vs=dtAlign4(sizeof(float)*3*h->vertCount),ps=dtAlign4(sizeof(dtPoly)*h->polyCount),ls=dtAlign4(sizeof(dtLink)*h->maxLinkCount),ds=dtAlign4(sizeof(dtPolyDetail)*h->detailMeshCount),dvs=dtAlign4(sizeof(float)*3*h->detailVertCount),dts=dtAlign4(4*h->detailTriCount),bs=dtAlign4(sizeof(dtBVNode)*h->bvNodeCount);
 auto verts=(float*)(bytes+hs);for(int i=0;i<h->vertCount;i++)for(int j=0;j<3;j++)verts[i*3+j]+=delta[j];auto dv=(float*)(bytes+hs+vs+ps+ls+ds);for(int i=0;i<h->detailVertCount;i++)for(int j=0;j<3;j++)dv[i*3+j]+=delta[j];auto off=(dtOffMeshConnection*)(bytes+hs+vs+ps+ls+ds+dvs+dts+bs);for(int i=0;i<h->offMeshConCount;i++)for(int j=0;j<6;j++)off[i].pos[j]+=delta[j%3];return validate_tile(bytes,size)?0:1;
}
int aife_nav_bake(const BakeConfig* cfg,const float* verts,int nv,const int* tris,int nt,const Link* links,int nl,unsigned char** out,int* out_size){
 if(!cfg||!verts||!tris||!out||!out_size||nv<3||nv>1000000||nt<1||nt>1000000||nl<0||nl>1024||(!links&&nl))return 1;
 const auto& c=*cfg;
 if(!std::isfinite(c.cs)||c.cs<0.025f||c.cs>1||!std::isfinite(c.ch)||c.ch<0.01f||c.ch>0.5f||!std::isfinite(c.width)||c.width<4||c.width>64||!finite3(c.bmin)||!finite3(c.bmax)||c.radius<0.05f||c.radius>4||c.height<0.2f||c.height>8||c.climb<0||c.climb>2||c.slope<1||c.slope>85)return 1;
 for(int i=0;i<nv*3;i++)if(!std::isfinite(verts[i])||std::abs(verts[i])>32768)return 1;
 for(int i=0;i<nt*3;i++)if(tris[i]<0||tris[i]>=nv)return 1;
 for(int i=0;i<nl;i++)if(!finite3(links[i].start)||!finite3(links[i].end)||!std::isfinite(links[i].radius)||links[i].radius<=0||links[i].radius>4)return 1;
 rcConfig rc{};rc.cs=c.cs;rc.ch=c.ch;rc.walkableSlopeAngle=c.slope;rc.walkableHeight=(int)std::ceil(c.height/c.ch);rc.walkableClimb=(int)std::floor(c.climb/c.ch);rc.walkableRadius=(int)std::ceil(c.radius/c.cs);rc.tileSize=(int)std::round(c.width/c.cs);rc.borderSize=rc.walkableRadius+3;rc.width=rc.height=rc.tileSize+rc.borderSize*2;if(rc.width>1024||rc.walkableHeight<3)return 1;
 rc.maxEdgeLen=(int)(12/c.cs);rc.maxSimplificationError=1.3f;rc.minRegionArea=0;rc.mergeRegionArea=16;rc.maxVertsPerPoly=6;rc.detailSampleDist=c.cs*6;rc.detailSampleMaxError=c.ch;
 rcVcopy(rc.bmin,c.bmin);rcVcopy(rc.bmax,c.bmax);rc.bmin[0]-=rc.borderSize*c.cs;rc.bmin[2]-=rc.borderSize*c.cs;rc.bmax[0]+=rc.borderSize*c.cs;rc.bmax[2]+=rc.borderSize*c.cs;
 rcContext ctx;
 std::unique_ptr<rcHeightfield,decltype(&rcFreeHeightField)> hf(rcAllocHeightfield(),rcFreeHeightField);
 if(!hf||!rcCreateHeightfield(&ctx,*hf,rc.width,rc.height,rc.bmin,rc.bmax,c.cs,c.ch))return 2;
 std::vector<unsigned char> areas(nt,0);rcMarkWalkableTriangles(&ctx,c.slope,verts,nv,tris,nt,areas.data());
 if(!rcRasterizeTriangles(&ctx,verts,nv,tris,areas.data(),nt,*hf,rc.walkableClimb))return 2;
 rcFilterLowHangingWalkableObstacles(&ctx,rc.walkableClimb,*hf);rcFilterLedgeSpans(&ctx,rc.walkableHeight,rc.walkableClimb,*hf);rcFilterWalkableLowHeightSpans(&ctx,rc.walkableHeight,*hf);
 std::unique_ptr<rcCompactHeightfield,decltype(&rcFreeCompactHeightfield)> chf(rcAllocCompactHeightfield(),rcFreeCompactHeightfield);
 if(!chf||!rcBuildCompactHeightfield(&ctx,rc.walkableHeight,rc.walkableClimb,*hf,*chf)||!rcErodeWalkableArea(&ctx,rc.walkableRadius,*chf)||!rcBuildRegionsMonotone(&ctx,*chf,rc.borderSize,rc.minRegionArea,rc.mergeRegionArea))return 2;
 hf.reset();std::unique_ptr<rcContourSet,decltype(&rcFreeContourSet)> contours(rcAllocContourSet(),rcFreeContourSet);
 if(!contours||!rcBuildContours(&ctx,*chf,rc.maxSimplificationError,rc.maxEdgeLen,*contours))return 2;
 if(contours->nconts==0)return 3;
 std::unique_ptr<rcPolyMesh,decltype(&rcFreePolyMesh)> poly(rcAllocPolyMesh(),rcFreePolyMesh);
 std::unique_ptr<rcPolyMeshDetail,decltype(&rcFreePolyMeshDetail)> detail(rcAllocPolyMeshDetail(),rcFreePolyMeshDetail);
 if(!poly||!detail||!rcBuildPolyMesh(&ctx,*contours,6,*poly)||!rcBuildPolyMeshDetail(&ctx,*poly,*chf,rc.detailSampleDist,rc.detailSampleMaxError,*detail)||poly->npolys>8192)return 2;
 for(int i=0;i<poly->npolys;i++){poly->areas[i]=0;poly->flags[i]=1;}
 std::vector<float> lv,lr;std::vector<unsigned char> ld(nl),la(nl,0);std::vector<unsigned short> lf(nl,1);std::vector<unsigned int> li(nl);
 for(int i=0;i<nl;i++){lv.insert(lv.end(),links[i].start,links[i].start+3);lv.insert(lv.end(),links[i].end,links[i].end+3);lr.push_back(links[i].radius);ld[i]=links[i].bidirectional?1:0;li[i]=links[i].id;}
 dtNavMeshCreateParams p{};p.verts=poly->verts;p.vertCount=poly->nverts;p.polys=poly->polys;p.polyAreas=poly->areas;p.polyFlags=poly->flags;p.polyCount=poly->npolys;p.nvp=poly->nvp;p.detailMeshes=detail->meshes;p.detailVerts=detail->verts;p.detailVertsCount=detail->nverts;p.detailTris=detail->tris;p.detailTriCount=detail->ntris;
 p.offMeshConVerts=lv.data();p.offMeshConRad=lr.data();p.offMeshConDir=ld.data();p.offMeshConAreas=la.data();p.offMeshConFlags=lf.data();p.offMeshConUserID=li.data();p.offMeshConCount=nl;p.walkableHeight=c.height;p.walkableRadius=c.radius;p.walkableClimb=c.climb;p.tileX=c.x;p.tileY=c.z;rcVcopy(p.bmin,poly->bmin);rcVcopy(p.bmax,poly->bmax);p.cs=c.cs;p.ch=c.ch;p.buildBvTree=true;
 unsigned char* data=nullptr;int size=0;if(!dtCreateNavMeshData(&p,&data,&size))return 2;if(!validate_tile(data,size)){dtFree(data);return 4;}*out=data;*out_size=size;return 0;
}
void* aife_nav_create(float width){
 if(!std::isfinite(width)||width<4||width>64)return nullptr;
 auto n=std::make_unique<NativeNav>();n->mesh=dtAllocNavMesh();n->query=dtAllocNavMeshQuery();if(!n->mesh||!n->query)return nullptr;dtNavMeshParams p{};p.tileWidth=width;p.tileHeight=width;p.maxTiles=256;p.maxPolys=8192;if(dtStatusFailed(n->mesh->init(&p))||dtStatusFailed(n->query->init(n->mesh,4096)))return nullptr;return n.release();
}
void aife_nav_destroy(void* ptr){delete (NativeNav*)ptr;}
int aife_nav_add(void* ptr,const unsigned char* bytes,int size,uint64_t* result){if(!ptr||!result||!bytes||size<1||size>16*1024*1024)return 1;auto n=(NativeNav*)ptr;auto copy=(unsigned char*)dtAlloc(size,DT_ALLOC_PERM);if(!copy)return 2;std::memcpy(copy,bytes,size);if(!validate_tile(copy,size)){dtFree(copy);return 1;}dtTileRef ref=0;auto status=n->mesh->addTile(copy,size,DT_TILE_FREE_DATA,0,&ref);if(dtStatusFailed(status)){dtFree(copy);return 2;}*result=ref;return 0;}
int aife_nav_remove(void* ptr,uint64_t ref){return !ptr||dtStatusFailed(((NativeNav*)ptr)->mesh->removeTile(ref,nullptr,nullptr))?1:0;}
int aife_nav_valid(void* ptr,uint64_t ref){dtQueryFilter filter;return ptr&&((NativeNav*)ptr)->query->isValidPolyRef(ref,&filter);}
int aife_nav_path(void* ptr,const float* from,const float* to,const float* extent,float* points,unsigned char* flags,uint64_t* refs,int* count,uint64_t* end_ref){
 if(!ptr||!from||!to||!extent||!points||!flags||!refs||!count||!end_ref||!finite3(from)||!finite3(to)||!finite3(extent))return 4;auto n=(NativeNav*)ptr;dtQueryFilter filter;dtPolyRef start=0,end=0;float a[3],b[3];
 if(dtStatusFailed(n->query->findNearestPoly(from,extent,&filter,&start,a))||dtStatusFailed(n->query->findNearestPoly(to,extent,&filter,&end,b))||!start||!end)return 2;
 dtPolyRef corridor[256];int nc=0;auto s=n->query->findPath(start,end,a,b,&filter,corridor,&nc,256);if(dtStatusFailed(s))return 4;if(dtStatusDetail(s,DT_OUT_OF_NODES)||dtStatusDetail(s,DT_BUFFER_TOO_SMALL))return 3;if(nc==0||corridor[nc-1]!=end)return 1;*end_ref=end;
 s=n->query->findStraightPath(a,b,corridor,nc,points,flags,(dtPolyRef*)refs,count,256);if(dtStatusFailed(s))return 4;if(dtStatusDetail(s,DT_BUFFER_TOO_SMALL))return 3;return 0;
}
int aife_nav_move(void* ptr,const float* from,const float* to,const float* extent,float* result){
 if(!ptr||!from||!to||!extent||!result||!finite3(from)||!finite3(to)||!finite3(extent))return 4;
 auto n=(NativeNav*)ptr;dtQueryFilter filter;dtPolyRef start=0;float a[3];
 if(dtStatusFailed(n->query->findNearestPoly(from,extent,&filter,&start,a))||!start)return 2;
 dtPolyRef visited[64];int count=0;auto s=n->query->moveAlongSurface(start,a,to,&filter,result,visited,&count,64);
 if(dtStatusFailed(s))return 4;if(dtStatusDetail(s,DT_BUFFER_TOO_SMALL))return 3;return 0;
}
// Conservative loaded-region obstacles: flag intersecting polygons, preserving
// the immutable cooked tile bytes. No global rebake and no hidden worker world.
int aife_nav_obstacles(void* ptr,const float* boxes,int count){
 if(!ptr||count<0||count>128||(!boxes&&count))return 1;auto n=(NativeNav*)ptr;
 for(int t=0;t<n->mesh->getMaxTiles();t++){const dtMeshTile* tile=((const dtNavMesh*)n->mesh)->getTile(t);if(!tile||!tile->header)continue;auto base=n->mesh->getPolyRefBase(tile);for(int p=0;p<tile->header->polyCount;p++)n->mesh->setPolyFlags(base|(dtPolyRef)p,1);}
 dtQueryFilter filter;
 for(int i=0;i<count;i++){const float* box=boxes+i*6;if(!finite3(box)||!finite3(box+3))return 1;float center[3],extent[3];for(int j=0;j<3;j++){if(box[j]>box[j+3])return 1;center[j]=(box[j]+box[j+3])*0.5f;extent[j]=(box[j+3]-box[j])*0.5f;}
 dtPolyRef refs[8192];int nr=0;auto s=n->query->queryPolygons(center,extent,&filter,refs,&nr,8192);if(dtStatusFailed(s)||dtStatusDetail(s,DT_BUFFER_TOO_SMALL))return 2;for(int k=0;k<nr;k++)n->mesh->setPolyFlags(refs[k],0);
 }return 0;
}
}
