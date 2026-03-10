package pact

import (
	"github.com/pact-foundation/pact-go/v2/consumer"
	message "github.com/pact-foundation/pact-go/v2/message/v4"
)

type V4 = consumer.V4HTTPMockProvider
type Interaction = consumer.V4InteractionWithPluginRequest
type PluginConfig = consumer.PluginConfig
type V4InteractionWithPluginRequestBuilder = consumer.V4InteractionWithPluginRequestBuilder

type MessagePact = message.AsynchronousPact
type MessageInteraction = message.AsynchronousMessageWithPluginContents
type MessagePluginConfig = message.PluginConfig
