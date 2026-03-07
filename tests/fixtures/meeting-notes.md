# Meeting Notes - 2024-03-15

## Sprint Planning

### Discussed Items

1. **Database migration timeline**
   - Decision: We will migrate to PostgreSQL 16 by end of Q2
   - John will lead the migration effort
   - Need to update all connection strings

2. **API versioning strategy**
   - Decision: Use URL path versioning (e.g., /v1/users, /v2/users)
   - Deprecation notice must be given 6 months in advance

3. **Performance optimization**
   - TODO: Profile the search endpoint - it's currently 3x slower than target
   - Constraint: Memory usage must stay below 4GB per instance

### Action Items

- [ ] John: Create migration plan document
- [ ] Sarah: Benchmark new caching strategy
- [ ] Team: Review API v2 specification

## 技术讨论

我们讨论了关于中文搜索的支持问题。目前系统对中文的分词效果不够理想，
需要引入更好的分词器。

**决定**：使用 jieba 分词器来处理中文文本。

**偏好**：优先使用本地模型而不是远程 API 来处理敏感数据。

**事实**：当前系统每天处理约 50 万条中文文本记录。
