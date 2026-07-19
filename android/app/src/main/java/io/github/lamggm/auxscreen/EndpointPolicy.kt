package io.github.lamggm.auxscreen

import java.net.URI

internal object EndpointPolicy {
    fun validate(endpoint: String, allowLanCleartext: Boolean): Result<String> = runCatching {
        val normalized = endpoint.trim().removeSuffix("/")
        val uri = URI(normalized)
        require(uri.scheme == "ws" || uri.scheme == "wss") {
            "O endereço precisa começar com ws:// ou wss://"
        }
        require(!uri.host.isNullOrBlank()) { "Informe um host válido" }
        require(uri.path == "/v1/session") { "O caminho precisa ser /v1/session" }
        if (uri.scheme == "ws") {
            require(allowLanCleartext) { "Esta variante exige wss://" }
            require(isPrivateNumericIpv4(uri.host)) {
                "ws:// só é permitido para um IPv4 privado da LAN"
            }
        }
        normalized
    }

    fun isPrivateNumericIpv4(host: String): Boolean {
        val parts = host.split('.')
        if (parts.size != 4) return false
        val octets = parts.map { it.toIntOrNull() ?: return false }
        if (octets.any { it !in 0..255 }) return false
        val (a, b) = octets
        return a == 10 ||
            (a == 172 && b in 16..31) ||
            (a == 192 && b == 168) ||
            a == 127 ||
            (a == 169 && b == 254)
    }
}
