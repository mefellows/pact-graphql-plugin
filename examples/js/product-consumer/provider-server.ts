import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { AddressInfo } from 'node:net';

import cors from 'cors';
import express from 'express';
import { ApolloServer } from '@apollo/server';
import { expressMiddleware } from '@apollo/server/express4';

const schema = readFileSync(resolve(__dirname, 'schema.graphql'), 'utf8');

const data = {
  categories: [
    { id: 'cat-1', name: 'Bags', slug: 'bags' },
    { id: 'cat-2', name: 'Travel', slug: 'travel' },
  ],
  products: [
    {
      id: 'prod-1',
      sku: 'SKU-TRAIL-001',
      name: 'Trail Backpack',
      description: 'Lightweight hiking pack',
      status: 'ACTIVE',
      categoryId: 'cat-1',
      tags: ['outdoor', 'hiking'],
      variantIds: ['var-1'],
      reviewIds: ['rev-1'],
    },
    {
      id: 'prod-2',
      sku: 'SKU-WEEK-004',
      name: 'Weekender Tote',
      description: 'Carry-on sized tote',
      status: 'DRAFT',
      categoryId: 'cat-2',
      tags: ['travel', 'weekend'],
      variantIds: ['var-2'],
      reviewIds: [],
    },
  ],
  variants: [
    {
      id: 'var-1',
      sku: 'SKU-TRAIL-001',
      name: 'Trail Backpack - Black',
      price: { list: { amount: 129.99, currency: 'USD' }, sale: null },
      inventory: { quantity: 42, updatedAt: '2026-03-08T12:00:00Z' },
    },
    {
      id: 'var-2',
      sku: 'SKU-WEEK-004',
      name: 'Weekender Tote - Sand',
      price: { list: { amount: 159.99, currency: 'USD' }, sale: null },
      inventory: { quantity: 18, updatedAt: '2026-03-08T12:00:00Z' },
    },
  ],
  customers: [
    {
      id: 'cust-1',
      email: 'alex@example.com',
      name: 'Alex',
      defaultShippingAddress: {
        line1: '123 Market St',
        line2: null,
        city: 'San Francisco',
        state: 'CA',
        postalCode: '94103',
        country: 'US',
      },
    },
  ],
  reviews: [
    {
      id: 'rev-1',
      rating: 'FIVE',
      title: 'Perfect for trips',
      body: 'Roomy and comfortable.',
      authorId: 'cust-1',
      createdAt: '2026-03-01T08:00:00Z',
    },
  ],
  orders: [
    {
      id: 'order-1',
      status: 'PLACED',
      customerId: 'cust-1',
      lineItems: [
        { productId: 'prod-1', variantId: 'var-1', quantity: 2 },
        { productId: 'prod-2', variantId: 'var-2', quantity: 1 },
      ],
      total: { amount: 289.97, currency: 'USD' },
      shipments: [
        {
          id: 'ship-1',
          status: 'PENDING',
          trackingNumber: null,
          carrier: null,
          address: {
            line1: '123 Market St',
            line2: null,
            city: 'San Francisco',
            state: 'CA',
            postalCode: '94103',
            country: 'US',
          },
          shippedAt: null,
          deliveredAt: null,
        },
      ],
      placedAt: '2026-03-08T09:00:00Z',
    },
  ],
};

const getProduct = (id: string) => data.products.find((product) => product.id === id);
const getVariant = (id: string) => data.variants.find((variant) => variant.id === id);
const getCategory = (id: string) => data.categories.find((category) => category.id === id);
const getCustomer = (id: string) => data.customers.find((customer) => customer.id === id);
const getOrder = (id: string) => data.orders.find((order) => order.id === id);

export function buildInventoryChangedEvent(variantId: string) {
  const variant = getVariant(variantId);
  if (!variant) {
    throw new Error('Variant not found');
  }
  return {
    subscription: 'InventoryChanged',
    variables: { variantId },
    data: { inventoryChanged: variant.inventory },
  };
}

const resolveNodeType = (obj: { id?: string }) => {
  if (!obj.id) {
    return null;
  }
  if (obj.id.startsWith('prod-')) {
    return 'Product';
  }
  if (obj.id.startsWith('var-')) {
    return 'Variant';
  }
  if (obj.id.startsWith('cat-')) {
    return 'Category';
  }
  if (obj.id.startsWith('rev-')) {
    return 'Review';
  }
  if (obj.id.startsWith('cust-')) {
    return 'Customer';
  }
  if (obj.id.startsWith('ship-')) {
    return 'Shipment';
  }
  if (obj.id.startsWith('order-')) {
    return 'Order';
  }
  return null;
};

const resolveSearchResultType = (obj: { id?: string }) => {
  if (!obj.id) {
    return null;
  }
  if (obj.id.startsWith('prod-')) {
    return 'Product';
  }
  if (obj.id.startsWith('cat-')) {
    return 'Category';
  }
  if (obj.id.startsWith('rev-')) {
    return 'Review';
  }
  return null;
};

const resolvers = {
  Query: {
    product: (_: unknown, args: { id: string }) => {
      if (args.id === '10') {
        return {
          id: '10',
          sku: 'SKU-10',
          name: 'product name',
          description: null,
          status: 'ACTIVE',
          categoryId: 'cat-1',
          tags: [],
          variantIds: [],
          reviewIds: [],
        };
      }
      return getProduct(args.id) ?? null;
    },
    products: (_: unknown, args: { status?: string; search?: string }) => {
      let results = data.products;
      if (args.status) {
        results = results.filter((product) => product.status === args.status);
      }
      if (args.search) {
        const term = args.search.toLowerCase();
        results = results.filter((product) =>
          [product.name, product.description ?? '', product.sku].some((field) =>
            field.toLowerCase().includes(term),
          ),
        );
      }
      return results;
    },
    search: (_: unknown, args: { text: string }) => {
      const term = args.text.toLowerCase();
      const productMatches = data.products.filter((product) =>
        product.name.toLowerCase().includes(term),
      );
      const categoryMatches = data.categories.filter((category) =>
        category.name.toLowerCase().includes(term),
      );
      const reviewMatches = data.reviews.filter((review) =>
        (review.title ?? '').toLowerCase().includes(term),
      );
      return [...productMatches, ...categoryMatches, ...reviewMatches];
    },
    order: (_: unknown, args: { id: string }) =>
      getOrder(args.id) ?? null,
  },
  Mutation: {
    addToCart: (
      _: unknown,
      args: { input: { productId: string; variantId?: string; quantity: number } },
    ) => {
      const product = getProduct(args.input.productId);
      if (!product) {
        throw new Error('Product not found');
      }
      const variantId = args.input.variantId ?? product.variantIds[0];
      if (!variantId) {
        throw new Error('Variant not found');
      }
      const variant = getVariant(variantId);
      if (!variant) {
        throw new Error('Variant not found');
      }
      return {
        product,
        variant,
        quantity: args.input.quantity,
        price: variant.price,
      };
    },
    placeOrder: (
      _: unknown,
      args: {
        input: {
          customerId: string;
          lines: Array<{ productId: string; variantId?: string; quantity: number }>;
          shippingAddress: {
            line1: string;
            line2?: string | null;
            city: string;
            state: string;
            postalCode: string;
            country: string;
          };
        };
      },
    ) => {
      const customer = getCustomer(args.input.customerId);
      if (!customer) {
        throw new Error('Customer not found');
      }

      const orderLines = args.input.lines.map((line) => {
        const product = getProduct(line.productId);
        if (!product) {
          throw new Error('Product not found');
        }
        const variantId = line.variantId ?? product.variantIds[0];
        if (!variantId) {
          throw new Error('Variant not found');
        }
        const variant = getVariant(variantId);
        if (!variant) {
          throw new Error('Variant not found');
        }
        return { productId: product.id, variantId: variant.id, quantity: line.quantity };
      });

      return {
        id: 'order-1',
        status: 'PLACED',
        customerId: customer.id,
        lineItems: orderLines,
        total: { amount: 289.97, currency: 'USD' },
        shipments: [
          {
            id: 'ship-1',
            status: 'PENDING',
            trackingNumber: null,
            carrier: null,
            address: {
              line1: args.input.shippingAddress.line1,
              line2: args.input.shippingAddress.line2 ?? null,
              city: args.input.shippingAddress.city,
              state: args.input.shippingAddress.state,
              postalCode: args.input.shippingAddress.postalCode,
              country: args.input.shippingAddress.country,
            },
            shippedAt: null,
            deliveredAt: null,
          },
        ],
        placedAt: '2026-03-08T09:00:00Z',
      };
    },
    submitReview: (
      _: unknown,
      args: { input: { productId: string; rating: string; title?: string | null; body?: string | null } },
    ) => {
      const product = getProduct(args.input.productId);
      if (!product) {
        throw new Error('Product not found');
      }
      const author = data.customers[0];
      return {
        id: 'rev-new',
        rating: args.input.rating,
        title: args.input.title ?? null,
        body: args.input.body ?? null,
        authorId: author.id,
        createdAt: '2026-03-08T10:00:00Z',
      };
    },
  },
  Product: {
    category: (product: { categoryId: string }) => getCategory(product.categoryId) ?? null,
    variants: (product: { variantIds: string[] }) =>
      data.variants.filter((variant) => product.variantIds.includes(variant.id)),
    reviews: (product: { reviewIds: string[] }) =>
      data.reviews.filter((review) => product.reviewIds.includes(review.id)),
  },
  Variant: {
    price: (variant: { price: unknown }) => variant.price,
    inventory: (variant: { inventory: unknown; id: string }) =>
      buildInventoryChangedEvent(variant.id).data.inventoryChanged,
  },
  Review: {
    author: (review: { authorId: string }) => {
      const author = getCustomer(review.authorId);
      if (!author) {
        throw new Error('Customer not found');
      }
      return author;
    },
  },
  Customer: {
    defaultShippingAddress: (customer: { defaultShippingAddress: unknown }) =>
      customer.defaultShippingAddress,
    addresses: (customer: { defaultShippingAddress: unknown }) =>
      customer.defaultShippingAddress ? [customer.defaultShippingAddress] : [],
  },
  Order: {
    customer: (order: { customerId: string }) => {
      const customer = getCustomer(order.customerId);
      if (!customer) {
        throw new Error('Customer not found');
      }
      return customer;
    },
    lines: (order: { lineItems: Array<{ productId: string; variantId: string; quantity: number }> }) =>
      order.lineItems.map((line) => {
        const product = getProduct(line.productId);
        if (!product) {
          throw new Error('Product not found');
        }
        const variant = getVariant(line.variantId);
        if (!variant) {
          throw new Error('Variant not found');
        }
        return {
          product,
          variant,
          quantity: line.quantity,
          price: variant.price,
        };
      }),
    total: (order: { total: unknown }) => order.total,
    shipments: (order: { shipments: unknown }) => order.shipments,
  },
  SearchResult: {
    __resolveType: resolveSearchResultType,
  },
  Node: {
    __resolveType: resolveNodeType,
  },
};

export async function startProviderServer() {
  const server = new ApolloServer({ typeDefs: schema, resolvers });
  await server.start();

  const app = express();
  app.use(
    '/graphql',
    cors<cors.CorsRequest>(),
    express.json({ type: ['application/json', 'application/graphql'] }),
    expressMiddleware(server),
  );

  const httpServer = app.listen(0);
  const { port } = httpServer.address() as AddressInfo;

  return {
    url: `http://127.0.0.1:${port}`,
    close: async () => {
      await server.stop();
      await new Promise<void>((resolve) => httpServer.close(() => resolve()));
    },
  };
}
