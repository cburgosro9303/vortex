//! Code generator implementations for various languages.

use vortex_domain::codegen::{CodeGenOptions, CodeLanguage, CodeSnippet};
use vortex_domain::request::RequestSpec;

/// Generate code for a request in the specified language.
#[must_use]
pub fn generate_code(request: &RequestSpec, options: &CodeGenOptions) -> CodeSnippet {
    let generator = CodeGenerator::new(options);
    generator.generate(request)
}

/// Code generator that produces code snippets from requests.
pub struct CodeGenerator<'a> {
    options: &'a CodeGenOptions,
}

impl<'a> CodeGenerator<'a> {
    /// Create a new code generator with the given options.
    #[must_use]
    pub const fn new(options: &'a CodeGenOptions) -> Self {
        Self { options }
    }

    /// Generate code for the given request.
    #[must_use]
    pub fn generate(&self, request: &RequestSpec) -> CodeSnippet {
        match self.options.language {
            CodeLanguage::Curl => self.generate_curl(request),
            CodeLanguage::Python => self.generate_python(request),
            CodeLanguage::JavaScript => self.generate_javascript_fetch(request),
            CodeLanguage::JavaScriptAxios => self.generate_javascript_axios(request),
            CodeLanguage::TypeScript => self.generate_typescript(request),
            CodeLanguage::Rust => self.generate_rust(request),
            CodeLanguage::Go => self.generate_go(request),
            CodeLanguage::Java => self.generate_java(request),
            CodeLanguage::CSharp => self.generate_csharp(request),
            CodeLanguage::Php => self.generate_php(request),
            CodeLanguage::Ruby => self.generate_ruby(request),
            CodeLanguage::Swift => self.generate_swift(request),
            CodeLanguage::Kotlin => self.generate_kotlin(request),
        }
    }

    fn generate_curl(&self, request: &RequestSpec) -> CodeSnippet {
        let mut parts = vec!["curl".to_string()];
        let url = request.full_url();

        // Method (GET is default, others need -X)
        if request.method != vortex_domain::request::HttpMethod::Get {
            parts.push(format!("-X {}", request.method.as_str().to_uppercase()));
        }

        // Headers
        for header in request.enabled_headers() {
            parts.push(format!("-H '{}: {}'", header.name, header.value));
        }

        // Body
        if !request.body.content.is_empty() {
            let escaped = request.body.content.replace('\'', "'\\''");
            parts.push(format!("-d '{}'", escaped));
        }

        // Content-Type from body
        if let Some(ct) = request.body.content_type() {
            let has_ct = request.headers.all().iter().any(|h| h.name.eq_ignore_ascii_case("content-type"));
            if !has_ct {
                parts.push(format!("-H 'Content-Type: {}'", ct));
            }
        }

        // URL (quoted)
        parts.push(format!("'{}'", url));

        let code = if self.options.pretty_format {
            parts.join(" \\\n  ")
        } else {
            parts.join(" ")
        };

        CodeSnippet::new(code, CodeLanguage::Curl)
    }

    fn generate_python(&self, request: &RequestSpec) -> CodeSnippet {
        let indent = self.options.indent();
        let url = request.full_url();
        let method = request.method.as_str().lower();

        let mut code = String::new();

        // Headers dict
        let headers: Vec<_> = request.enabled_headers().collect();
        if !headers.is_empty() || request.body.content_type().is_some() {
            code.push_str("headers = {\n");
            for h in &headers {
                code.push_str(&format!("{}'{}: '{}',\n", indent, h.name, h.value));
            }
            if let Some(ct) = request.body.content_type() {
                let has_ct = headers.iter().any(|h| h.name.eq_ignore_ascii_case("content-type"));
                if !has_ct {
                    code.push_str(&format!("{}'Content-Type': '{}',\n", indent, ct));
                }
            }
            code.push_str("}\n\n");
        }

        // Body
        if !request.body.content.is_empty() {
            code.push_str(&format!("data = '''{}'''\n\n", request.body.content));
        }

        // Request
        code.push_str(&format!("response = requests.{}(\n", method));
        code.push_str(&format!("{}'{}',\n", indent, url));

        if !headers.is_empty() || request.body.content_type().is_some() {
            code.push_str(&format!("{}headers=headers,\n", indent));
        }

        if !request.body.content.is_empty() {
            code.push_str(&format!("{}data=data,\n", indent));
        }

        code.push_str(")\n\n");
        code.push_str("print(response.status_code)\nprint(response.text)");

        CodeSnippet::new(code, CodeLanguage::Python)
            .with_import("import requests")
    }

    fn generate_javascript_fetch(&self, request: &RequestSpec) -> CodeSnippet {
        let indent = self.options.indent();
        let url = request.full_url();

        let mut code = String::new();

        if self.options.use_async {
            code.push_str("const response = await fetch('");
        } else {
            code.push_str("fetch('");
        }
        code.push_str(&url);
        code.push_str("', {\n");

        // Method
        code.push_str(&format!("{}method: '{}',\n", indent, request.method.as_str().to_uppercase()));

        // Headers
        let headers: Vec<_> = request.enabled_headers().collect();
        if !headers.is_empty() || request.body.content_type().is_some() {
            code.push_str(&format!("{}headers: {{\n", indent));
            for h in &headers {
                code.push_str(&format!("{}{}'{}': '{}',\n", indent, indent, h.name, h.value));
            }
            if let Some(ct) = request.body.content_type() {
                let has_ct = headers.iter().any(|h| h.name.eq_ignore_ascii_case("content-type"));
                if !has_ct {
                    code.push_str(&format!("{}{}'Content-Type': '{}',\n", indent, indent, ct));
                }
            }
            code.push_str(&format!("{}}},\n", indent));
        }

        // Body
        if !request.body.content.is_empty() {
            let escaped = request.body.content.replace('\\', "\\\\").replace('`', "\\`");
            code.push_str(&format!("{}body: `{}`,\n", indent, escaped));
        }

        code.push_str("});\n\n");

        if self.options.use_async {
            code.push_str("const data = await response.json();\nconsole.log(data);");
        } else {
            code.push_str(".then(response => response.json())\n.then(data => console.log(data));");
        }

        CodeSnippet::new(code, CodeLanguage::JavaScript)
    }

    fn generate_javascript_axios(&self, request: &RequestSpec) -> CodeSnippet {
        let indent = self.options.indent();
        let url = request.full_url();
        let method = request.method.as_str().lower();

        let mut code = String::new();

        if self.options.use_async {
            code.push_str("const response = await axios({\n");
        } else {
            code.push_str("axios({\n");
        }

        code.push_str(&format!("{}method: '{}',\n", indent, method));
        code.push_str(&format!("{}url: '{}',\n", indent, url));

        // Headers
        let headers: Vec<_> = request.enabled_headers().collect();
        if !headers.is_empty() {
            code.push_str(&format!("{}headers: {{\n", indent));
            for h in &headers {
                code.push_str(&format!("{}{}'{}': '{}',\n", indent, indent, h.name, h.value));
            }
            code.push_str(&format!("{}}},\n", indent));
        }

        // Body
        if !request.body.content.is_empty() {
            code.push_str(&format!("{}data: {},\n", indent, request.body.content));
        }

        code.push_str("})");

        if self.options.use_async {
            code.push_str(";\n\nconsole.log(response.data);");
        } else {
            code.push_str("\n.then(response => console.log(response.data));");
        }

        CodeSnippet::new(code, CodeLanguage::JavaScriptAxios)
            .with_import("const axios = require('axios');")
    }

    fn generate_typescript(&self, request: &RequestSpec) -> CodeSnippet {
        let indent = self.options.indent();
        let url = request.full_url();

        let mut code = String::new();

        code.push_str("const response: Response = await fetch('");
        code.push_str(&url);
        code.push_str("', {\n");

        code.push_str(&format!("{}method: '{}',\n", indent, request.method.as_str().to_uppercase()));

        // Headers
        let headers: Vec<_> = request.enabled_headers().collect();
        if !headers.is_empty() || request.body.content_type().is_some() {
            code.push_str(&format!("{}headers: {{\n", indent));
            for h in &headers {
                code.push_str(&format!("{}{}'{}': '{}',\n", indent, indent, h.name, h.value));
            }
            if let Some(ct) = request.body.content_type() {
                let has_ct = headers.iter().any(|h| h.name.eq_ignore_ascii_case("content-type"));
                if !has_ct {
                    code.push_str(&format!("{}{}'Content-Type': '{}',\n", indent, indent, ct));
                }
            }
            code.push_str(&format!("{}}},\n", indent));
        }

        if !request.body.content.is_empty() {
            let escaped = request.body.content.replace('\\', "\\\\").replace('`', "\\`");
            code.push_str(&format!("{}body: `{}`,\n", indent, escaped));
        }

        code.push_str("});\n\n");
        code.push_str("const data = await response.json();\nconsole.log(data);");

        CodeSnippet::new(code, CodeLanguage::TypeScript)
    }

    fn generate_rust(&self, request: &RequestSpec) -> CodeSnippet {
        let indent = self.options.indent();
        let url = request.full_url();
        let method = request.method.as_str().lower();

        let mut code = String::new();

        code.push_str("let client = reqwest::Client::new();\n\n");
        code.push_str(&format!("let response = client.{}(\"{}\")\n", method, url));

        // Headers
        for h in request.enabled_headers() {
            code.push_str(&format!("{}.header(\"{}\", \"{}\")\n", indent, h.name, h.value));
        }

        // Body
        if !request.body.content.is_empty() {
            let escaped = request.body.content.replace('\\', "\\\\").replace('"', "\\\"");
            code.push_str(&format!("{}.body(\"{}\")\n", indent, escaped));
        }

        code.push_str(&format!("{}.send()\n", indent));
        code.push_str(&format!("{}.await?;\n\n", indent));

        code.push_str("println!(\"Status: {}\", response.status());\n");
        code.push_str("let body = response.text().await?;\nprintln!(\"{}\", body);");

        CodeSnippet::new(code, CodeLanguage::Rust)
            .with_imports(["use reqwest;", "use std::error::Error;"])
    }

    fn generate_go(&self, request: &RequestSpec) -> CodeSnippet {
        let indent = self.options.indent();
        let url = request.full_url();
        let method = request.method.as_str().to_uppercase();

        let mut code = String::new();

        // Body setup
        if !request.body.content.is_empty() {
            code.push_str(&format!("body := strings.NewReader(`{}`)\n", request.body.content));
            code.push_str(&format!("req, err := http.NewRequest(\"{}\", \"{}\", body)\n", method, url));
        } else {
            code.push_str(&format!("req, err := http.NewRequest(\"{}\", \"{}\", nil)\n", method, url));
        }

        code.push_str("if err != nil {\n");
        code.push_str(&format!("{}log.Fatal(err)\n", indent));
        code.push_str("}\n\n");

        // Headers
        for h in request.enabled_headers() {
            code.push_str(&format!("req.Header.Set(\"{}\", \"{}\")\n", h.name, h.value));
        }

        if let Some(ct) = request.body.content_type() {
            let has_ct = request.headers.all().iter().any(|h| h.name.eq_ignore_ascii_case("content-type"));
            if !has_ct {
                code.push_str(&format!("req.Header.Set(\"Content-Type\", \"{}\")\n", ct));
            }
        }

        code.push_str("\nclient := &http.Client{}\n");
        code.push_str("resp, err := client.Do(req)\n");
        code.push_str("if err != nil {\n");
        code.push_str(&format!("{}log.Fatal(err)\n", indent));
        code.push_str("}\n");
        code.push_str("defer resp.Body.Close()\n\n");

        code.push_str("responseBody, _ := io.ReadAll(resp.Body)\n");
        code.push_str("fmt.Println(string(responseBody))");

        CodeSnippet::new(code, CodeLanguage::Go)
            .with_imports([
                "import (",
                "    \"fmt\"",
                "    \"io\"",
                "    \"log\"",
                "    \"net/http\"",
                "    \"strings\"",
                ")",
            ])
    }

    fn generate_java(&self, request: &RequestSpec) -> CodeSnippet {
        let indent = self.options.indent();
        let url = request.full_url();
        let method = request.method.as_str().to_uppercase();

        let mut code = String::new();

        code.push_str("HttpClient client = HttpClient.newHttpClient();\n\n");

        code.push_str("HttpRequest request = HttpRequest.newBuilder()\n");
        code.push_str(&format!("{}.uri(URI.create(\"{}\"))\n", indent, url));
        code.push_str(&format!("{}.method(\"{}\", ", indent, method));

        if !request.body.content.is_empty() {
            let escaped = request.body.content.replace('\\', "\\\\").replace('"', "\\\"");
            code.push_str(&format!("HttpRequest.BodyPublishers.ofString(\"{}\"))\n", escaped));
        } else {
            code.push_str("HttpRequest.BodyPublishers.noBody())\n");
        }

        // Headers
        for h in request.enabled_headers() {
            code.push_str(&format!("{}.header(\"{}\", \"{}\")\n", indent, h.name, h.value));
        }

        if let Some(ct) = request.body.content_type() {
            let has_ct = request.headers.all().iter().any(|h| h.name.eq_ignore_ascii_case("content-type"));
            if !has_ct {
                code.push_str(&format!("{}.header(\"Content-Type\", \"{}\")\n", indent, ct));
            }
        }

        code.push_str(&format!("{}.build();\n\n", indent));

        code.push_str("HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());\n\n");
        code.push_str("System.out.println(response.statusCode());\n");
        code.push_str("System.out.println(response.body());");

        CodeSnippet::new(code, CodeLanguage::Java)
            .with_imports([
                "import java.net.URI;",
                "import java.net.http.HttpClient;",
                "import java.net.http.HttpRequest;",
                "import java.net.http.HttpResponse;",
            ])
    }

    fn generate_csharp(&self, request: &RequestSpec) -> CodeSnippet {
        let indent = self.options.indent();
        let url = request.full_url();
        let method = match request.method {
            vortex_domain::request::HttpMethod::Get => "HttpMethod.Get",
            vortex_domain::request::HttpMethod::Post => "HttpMethod.Post",
            vortex_domain::request::HttpMethod::Put => "HttpMethod.Put",
            vortex_domain::request::HttpMethod::Patch => "HttpMethod.Patch",
            vortex_domain::request::HttpMethod::Delete => "HttpMethod.Delete",
            vortex_domain::request::HttpMethod::Head => "HttpMethod.Head",
            vortex_domain::request::HttpMethod::Options => "HttpMethod.Options",
        };

        let mut code = String::new();

        code.push_str("using var client = new HttpClient();\n\n");

        code.push_str("var request = new HttpRequestMessage\n{\n");
        code.push_str(&format!("{}Method = {},\n", indent, method));
        code.push_str(&format!("{}RequestUri = new Uri(\"{}\"),\n", indent, url));

        if !request.body.content.is_empty() {
            let ct = request.body.content_type().unwrap_or("text/plain");
            let escaped = request.body.content.replace('\\', "\\\\").replace('"', "\\\"");
            code.push_str(&format!("{}Content = new StringContent(\"{}\", Encoding.UTF8, \"{}\"),\n", indent, escaped, ct));
        }

        code.push_str("};\n\n");

        // Headers
        for h in request.enabled_headers() {
            code.push_str(&format!("request.Headers.Add(\"{}\", \"{}\");\n", h.name, h.value));
        }

        code.push_str("\nvar response = await client.SendAsync(request);\n");
        code.push_str("var content = await response.Content.ReadAsStringAsync();\n\n");
        code.push_str("Console.WriteLine(response.StatusCode);\n");
        code.push_str("Console.WriteLine(content);");

        CodeSnippet::new(code, CodeLanguage::CSharp)
            .with_imports([
                "using System.Net.Http;",
                "using System.Text;",
            ])
    }

    fn generate_php(&self, request: &RequestSpec) -> CodeSnippet {
        let url = request.full_url();
        let method = request.method.as_str().to_uppercase();

        let mut code = String::new();

        code.push_str("<?php\n\n");
        code.push_str("$ch = curl_init();\n\n");

        code.push_str(&format!("curl_setopt($ch, CURLOPT_URL, '{}');\n", url));
        code.push_str("curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);\n");

        if method != "GET" {
            code.push_str(&format!("curl_setopt($ch, CURLOPT_CUSTOMREQUEST, '{}');\n", method));
        }

        // Headers
        let headers: Vec<_> = request.enabled_headers().collect();
        if !headers.is_empty() || request.body.content_type().is_some() {
            code.push_str("\n$headers = [\n");
            for h in &headers {
                code.push_str(&format!("    '{}: {}',\n", h.name, h.value));
            }
            if let Some(ct) = request.body.content_type() {
                let has_ct = headers.iter().any(|h| h.name.eq_ignore_ascii_case("content-type"));
                if !has_ct {
                    code.push_str(&format!("    'Content-Type: {}',\n", ct));
                }
            }
            code.push_str("];\n");
            code.push_str("curl_setopt($ch, CURLOPT_HTTPHEADER, $headers);\n");
        }

        // Body
        if !request.body.content.is_empty() {
            let escaped = request.body.content.replace('\'', "\\'");
            code.push_str(&format!("\ncurl_setopt($ch, CURLOPT_POSTFIELDS, '{}');\n", escaped));
        }

        code.push_str("\n$response = curl_exec($ch);\n");
        code.push_str("$httpCode = curl_getinfo($ch, CURLINFO_HTTP_CODE);\n");
        code.push_str("curl_close($ch);\n\n");

        code.push_str("echo \"Status: $httpCode\\n\";\n");
        code.push_str("echo $response;");

        CodeSnippet::new(code, CodeLanguage::Php)
    }

    fn generate_ruby(&self, request: &RequestSpec) -> CodeSnippet {
        let url = request.full_url();

        let mut code = String::new();

        code.push_str(&format!("uri = URI('{}')\n", url));
        code.push_str("http = Net::HTTP.new(uri.host, uri.port)\n");
        code.push_str("http.use_ssl = uri.scheme == 'https'\n\n");

        let request_class = match request.method {
            vortex_domain::request::HttpMethod::Get => "Get",
            vortex_domain::request::HttpMethod::Post => "Post",
            vortex_domain::request::HttpMethod::Put => "Put",
            vortex_domain::request::HttpMethod::Patch => "Patch",
            vortex_domain::request::HttpMethod::Delete => "Delete",
            vortex_domain::request::HttpMethod::Head => "Head",
            vortex_domain::request::HttpMethod::Options => "Options",
        };

        code.push_str(&format!("request = Net::HTTP::{}::new(uri)\n", request_class));

        // Headers
        for h in request.enabled_headers() {
            code.push_str(&format!("request['{}'] = '{}'\n", h.name, h.value));
        }

        if let Some(ct) = request.body.content_type() {
            let has_ct = request.headers.all().iter().any(|h| h.name.eq_ignore_ascii_case("content-type"));
            if !has_ct {
                code.push_str(&format!("request['Content-Type'] = '{}'\n", ct));
            }
        }

        // Body
        if !request.body.content.is_empty() {
            let escaped = request.body.content.replace('\'', "\\'");
            code.push_str(&format!("request.body = '{}'\n", escaped));
        }

        code.push_str("\nresponse = http.request(request)\n\n");
        code.push_str("puts response.code\n");
        code.push_str("puts response.body");

        CodeSnippet::new(code, CodeLanguage::Ruby)
            .with_imports(["require 'net/http'", "require 'uri'"])
    }

    fn generate_swift(&self, request: &RequestSpec) -> CodeSnippet {
        let indent = self.options.indent();
        let url = request.full_url();
        let method = request.method.as_str().to_uppercase();

        let mut code = String::new();

        code.push_str(&format!("let url = URL(string: \"{}\")!\n", url));
        code.push_str("var request = URLRequest(url: url)\n");
        code.push_str(&format!("request.httpMethod = \"{}\"\n", method));

        // Headers
        for h in request.enabled_headers() {
            code.push_str(&format!("request.setValue(\"{}\", forHTTPHeaderField: \"{}\")\n", h.value, h.name));
        }

        if let Some(ct) = request.body.content_type() {
            let has_ct = request.headers.all().iter().any(|h| h.name.eq_ignore_ascii_case("content-type"));
            if !has_ct {
                code.push_str(&format!("request.setValue(\"{}\", forHTTPHeaderField: \"Content-Type\")\n", ct));
            }
        }

        // Body
        if !request.body.content.is_empty() {
            let escaped = request.body.content.replace('\\', "\\\\").replace('"', "\\\"");
            code.push_str(&format!("request.httpBody = \"{}\".data(using: .utf8)\n", escaped));
        }

        code.push_str("\nlet task = URLSession.shared.dataTask(with: request) { data, response, error in\n");
        code.push_str(&format!("{}if let error = error {{\n", indent));
        code.push_str(&format!("{}{}print(\"Error: \\(error)\")\n", indent, indent));
        code.push_str(&format!("{}{}return\n", indent, indent));
        code.push_str(&format!("{}}}\n", indent));
        code.push_str(&format!("{}if let data = data, let string = String(data: data, encoding: .utf8) {{\n", indent));
        code.push_str(&format!("{}{}print(string)\n", indent, indent));
        code.push_str(&format!("{}}}\n", indent));
        code.push_str("}\n");
        code.push_str("task.resume()");

        CodeSnippet::new(code, CodeLanguage::Swift)
            .with_import("import Foundation")
    }

    fn generate_kotlin(&self, request: &RequestSpec) -> CodeSnippet {
        let indent = self.options.indent();
        let url = request.full_url();

        let mut code = String::new();

        code.push_str("val client = OkHttpClient()\n\n");

        // Body
        if !request.body.content.is_empty() {
            let ct = request.body.content_type().unwrap_or("text/plain");
            let escaped = request.body.content.replace('\\', "\\\\").replace('"', "\\\"");
            code.push_str(&format!("val body = \"{}\".toRequestBody(\"{}\".toMediaType())\n\n", escaped, ct));
        }

        code.push_str("val request = Request.Builder()\n");
        code.push_str(&format!("{}.url(\"{}\")\n", indent, url));

        // Method with body
        let method = request.method.as_str().lower();
        if !request.body.content.is_empty() {
            code.push_str(&format!("{}.{}(body)\n", indent, method));
        } else if method != "get" {
            code.push_str(&format!("{}.{}(null)\n", indent, method));
        }

        // Headers
        for h in request.enabled_headers() {
            code.push_str(&format!("{}.addHeader(\"{}\", \"{}\")\n", indent, h.name, h.value));
        }

        code.push_str(&format!("{}.build()\n\n", indent));

        code.push_str("client.newCall(request).execute().use { response ->\n");
        code.push_str(&format!("{}println(response.code)\n", indent));
        code.push_str(&format!("{}println(response.body?.string())\n", indent));
        code.push_str("}");

        CodeSnippet::new(code, CodeLanguage::Kotlin)
            .with_imports([
                "import okhttp3.MediaType.Companion.toMediaType",
                "import okhttp3.OkHttpClient",
                "import okhttp3.Request",
                "import okhttp3.RequestBody.Companion.toRequestBody",
            ])
    }
}

/// Extension trait for lowercase conversion.
trait LowerExt {
    fn lower(&self) -> String;
}

impl LowerExt for &str {
    fn lower(&self) -> String {
        self.to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vortex_domain::request::Header;

    fn sample_request() -> RequestSpec {
        let mut req = RequestSpec::get("https://api.example.com/users");
        req.headers.add(Header::new("Authorization", "Bearer token123"));
        req
    }

    #[test]
    fn test_generate_curl() {
        let req = sample_request();
        let options = CodeGenOptions::for_language(CodeLanguage::Curl);
        let snippet = generate_code(&req, &options);

        assert!(snippet.code.contains("curl"));
        assert!(snippet.code.contains("https://api.example.com/users"));
        assert!(snippet.code.contains("Authorization"));
    }

    #[test]
    fn test_generate_python() {
        let req = sample_request();
        let options = CodeGenOptions::for_language(CodeLanguage::Python);
        let snippet = generate_code(&req, &options);

        assert!(snippet.code.contains("requests.get"));
        assert!(snippet.code.contains("https://api.example.com/users"));
        assert!(snippet.imports.iter().any(|i| i.contains("import requests")));
    }

    #[test]
    fn test_generate_javascript_fetch() {
        let req = sample_request();
        let options = CodeGenOptions::for_language(CodeLanguage::JavaScript);
        let snippet = generate_code(&req, &options);

        assert!(snippet.code.contains("fetch"));
        assert!(snippet.code.contains("https://api.example.com/users"));
    }

    #[test]
    fn test_generate_rust() {
        let req = sample_request();
        let options = CodeGenOptions::for_language(CodeLanguage::Rust);
        let snippet = generate_code(&req, &options);

        assert!(snippet.code.contains("reqwest"));
        assert!(snippet.code.contains("https://api.example.com/users"));
    }

    #[test]
    fn test_generate_with_body() {
        let mut req = RequestSpec::post("https://api.example.com/users");
        req.body = vortex_domain::request::RequestBody::json(r#"{"name": "test"}"#);

        let options = CodeGenOptions::for_language(CodeLanguage::Curl);
        let snippet = generate_code(&req, &options);

        assert!(snippet.code.contains("-X POST"));
        assert!(snippet.code.contains("-d"));
    }

    #[test]
    fn test_all_languages_generate() {
        let req = sample_request();

        for lang in CodeLanguage::all() {
            let options = CodeGenOptions::for_language(*lang);
            let snippet = generate_code(&req, &options);
            assert!(!snippet.code.is_empty(), "Empty code for {:?}", lang);
        }
    }
}
